use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::{Duration, Instant};

use replicant_game::enemies::shadow_matrix;
use replicant_game::npcs::{
    MirroredNpc, NPC_EVENT_SCENE_SETTLE_MS, NPC_MIRROR_DELAY_MS, NPC_SEND_INTERVAL_MS, NpcControl,
    NpcEvent, NpcHoldBlocker, NpcMirror, NpcPlanner, NpcRoster, NpcSighting, OwnTalks,
    PartnerTalks, npc_control, npc_motion_records_readable, npc_state, npc_sync_settling,
    read_npc_body_layer, read_npc_hold_blocker, scan_npcs, split_npc_snapshot,
};
use replicant_game::offsets::{
    ACTOR_DRAWN_FLAG_OFFSET, ACTOR_SPAWN_SERIAL_OFFSET, ACTOR_TRANSFORM_BLOCK_BYTES,
    ACTOR_TRANSFORM_BLOCK_OFFSET,
};
use replicant_game::puppet::{
    MOTION_BLEND_SECONDS, MotionStep, actor_is_live, follow_step, frame_to_seek, seek_layer_frame,
};
use replicant_game::{GameMemory, PhaseEntry, Replicant};
use replicant_proto::{Message, MotionState, NpcId, NpcState};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::memory::InProcessMemory;
use crate::net::{self, NetThread, RemotePlayer};
use crate::npcs::game_calls::NpcCalls;
use crate::npcs::hold_hook::{is_installed, set_held};
use crate::npcs::talk_guard_hook::set_partner_talks;

static SYNC: LazyLock<Mutex<NpcSync>> = LazyLock::new(|| Mutex::new(NpcSync::default()));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NpcRole {
    Host,
    Guest,
}

#[derive(Default)]
struct NpcSync {
    role: Option<NpcRole>,
    roster: NpcRoster,
    mirror: NpcMirror,
    planner: NpcPlanner,
    partner_talks: PartnerTalks,
    own_talks: OwnTalks,
    epoch: Option<Instant>,
    last_host_send: Option<Instant>,
    tick: u32,
    hidden_by_us: HashMap<u64, i32>,
    last_started: HashMap<NpcId, i32>,
    blocker: Option<NpcHoldBlocker>,
    event_scene_ended_at_ms: Option<u64>,
    refused_motion_actors: BTreeSet<u64>,
    logged_settling: bool,
    last_report: Option<NpcReport>,
    own_talking: BTreeSet<NpcId>,
    partner_talking: BTreeSet<NpcId>,
    idle_reason: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct NpcReport {
    local: usize,
    with_motion: usize,
    own_logic: usize,
    partner_talk: usize,
    driven: usize,
    hidden: usize,
    missing_here: usize,
}

struct LocalView<'a> {
    game: &'a Replicant<InProcessMemory>,
    build: &'a VerifiedBuild,
    calls: NpcCalls,
    phase: u16,
}

struct Pose {
    position: [f32; 3],
    yaw_radians: f32,
    motion: Option<MotionState>,
}

impl NpcSync {
    fn go_idle(&mut self, build: &VerifiedBuild, reason: &'static str) {
        self.release_all(build);
        self.role = None;
        self.reset_world();
        if self.idle_reason != Some(reason) {
            self.idle_reason = Some(reason);
            log_line(&format!("npcs: idle, {reason}"));
        }
    }

    fn reset_world(&mut self) {
        self.roster.reset();
        self.mirror.clear();
        self.planner.reset();
        self.partner_talks.clear();
        self.own_talks.clear();
        self.own_talking.clear();
        self.partner_talking.clear();
        self.last_host_send = None;
        self.last_report = None;
    }

    fn take_role(&mut self, build: &VerifiedBuild, role: NpcRole) {
        self.idle_reason = None;
        if self.role != Some(role) {
            self.release_all(build);
            self.reset_world();
            self.role = Some(role);
            log_line(&format!(
                "npcs: {} (hold hook {})",
                match role {
                    NpcRole::Host => "host, sending this game's NPCs",
                    NpcRole::Guest => "guest, following the host's NPCs",
                },
                if is_installed() { "on" } else { "off" }
            ));
        }
    }

    fn now_ms(&mut self) -> u64 {
        let epoch = *self.epoch.get_or_insert_with(Instant::now);
        u64::try_from(epoch.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn release_all(&mut self, build: &VerifiedBuild) {
        set_held(HashSet::new());
        set_partner_talks(HashSet::new(), "");
        self.last_started.clear();
        let mut memory = InProcessMemory::new(build);
        for (actor, serial) in self.hidden_by_us.drain() {
            show_again(&mut memory, actor, serial);
        }
    }
}

pub fn tick_npc_sync(build: &VerifiedBuild) {
    let mut sync = SYNC.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(net) = net::handle() else {
        sync.go_idle(build, "co-op is off");
        return;
    };
    let relayed = net
        .take_relayed(|message| matches!(message, Message::NpcSnapshot(_) | Message::NpcTalk(_)));
    let view = net.view();
    let Some(remote) = view.partner_state().cloned() else {
        sync.go_idle(build, "no other player is connected");
        return;
    };
    let role = if remote.slot == 1 {
        NpcRole::Host
    } else {
        NpcRole::Guest
    };
    let game = Replicant::new(InProcessMemory::new(build));
    let Some(phase) = local_phase(&game) else {
        sync.go_idle(build, "own phase not readable (loading?)");
        return;
    };
    sync.take_role(build, role);
    let local = LocalView {
        game: &game,
        build,
        calls: NpcCalls::new(build),
        phase,
    };
    step(&mut sync, net, &local, &remote, relayed);
}

fn local_phase(game: &Replicant<InProcessMemory>) -> Option<u16> {
    let name = game.current_phase().ok()?;
    u16::try_from(PhaseEntry::index_of(&name)?).ok()
}

fn step(
    sync: &mut NpcSync,
    net: &NetThread,
    local: &LocalView<'_>,
    remote: &RemotePlayer,
    relayed: Vec<Message>,
) {
    let now_ms = sync.now_ms();
    let is_host = sync.role == Some(NpcRole::Host);
    for message in relayed {
        match message {
            Message::NpcSnapshot(snapshot) if !is_host => {
                sync.mirror.receive_snapshot(now_ms, &snapshot);
            }
            Message::NpcTalk(talk) => sync.partner_talks.receive(now_ms, &talk),
            _ => {}
        }
    }
    sync.mirror.prune(now_ms);
    let sightings = scan_npcs(local.game.memory()).unwrap_or_default();
    let update = sync.roster.observe(local.phase, &sightings);
    if update.phase_changed {
        sync.planner.reset();
        sync.last_started.clear();
    }
    if update.last_phase_leftovers_found > 0 {
        log_line(&format!(
            "npcs: the phase changed while the game still holds {} NPCs of the last phase, they get no id in this phase",
            update.last_phase_leftovers_found
        ));
    }
    if update.last_phase_leftovers_gone > 0 {
        log_line(&format!(
            "npcs: {} NPCs of the last phase are gone now",
            update.last_phase_leftovers_gone
        ));
    }
    if is_host && (update.phase_changed || !update.arrived.is_empty() || !update.gone.is_empty()) {
        log_line(&format!(
            "npcs: host phase {} has {} NPCs ({} new, {} gone)",
            local.phase,
            update.seen.len(),
            update.arrived.len(),
            update.gone.len()
        ));
    }
    let same_phase = remote.state.phase == local.phase;
    send_own_talks(sync, net, now_ms, local.phase, &update.seen, same_phase);
    if is_host && same_phase {
        send_snapshot(sync, net, local, remote, &update.seen);
    }
    let blocker = read_npc_hold_blocker(local.game.memory()).ok().flatten();
    if blocker != sync.blocker {
        log_line(match blocker {
            Some(NpcHoldBlocker::EventScene) => "npcs: event scene, NPCs run on their own",
            None => "npcs: no event scene, NPC sync runs",
        });
        if blocker.is_none() {
            sync.event_scene_ended_at_ms = Some(now_ms);
        }
        sync.blocker = blocker;
    }
    if !is_host && blocker.is_none() && npc_sync_settling(now_ms, sync.event_scene_ended_at_ms) {
        if !sync.logged_settling {
            log_line(&format!(
                "npcs: event scene just ended, NPC sync waits {NPC_EVENT_SCENE_SETTLE_MS} ms for the game's own scene clean-up"
            ));
            sync.logged_settling = true;
        }
        sync.release_all(local.build);
        return;
    }
    sync.logged_settling = false;
    if blocker.is_some() || !same_phase {
        sync.release_all(local.build);
        return;
    }
    let partner = sync.partner_talks.active(now_ms, local.phase);
    note_partner_talks(sync, &partner);
    apply_control(
        sync,
        local,
        now_ms,
        is_host,
        &update.seen,
        &partner,
        &remote.name,
    );
}

fn send_own_talks(
    sync: &mut NpcSync,
    net: &NetThread,
    now_ms: u64,
    phase: u16,
    seen: &[(NpcId, NpcSighting)],
    same_phase: bool,
) {
    let talking: Vec<NpcState> = seen
        .iter()
        .filter(|(_, sighting)| sighting.event == Some(NpcEvent::Talk))
        .filter_map(|(id, sighting)| npc_state(*id, sighting))
        .collect();
    let now: BTreeSet<NpcId> = talking.iter().map(|npc| npc.id).collect();
    for id in now.difference(&sync.own_talking) {
        log_line(&format!(
            "npcs: talking to {} here, the other game holds it",
            id_text(*id)
        ));
    }
    for id in sync.own_talking.difference(&now) {
        log_line(&format!("npcs: talk with {} here ended", id_text(*id)));
    }
    sync.own_talking = now;
    let sending: &[NpcState] = if same_phase { &talking } else { &[] };
    for talk in sync.own_talks.update(now_ms, phase, sending) {
        net.send_relayed(Message::NpcTalk(talk));
    }
}

fn send_snapshot(
    sync: &mut NpcSync,
    net: &NetThread,
    local: &LocalView<'_>,
    remote: &RemotePlayer,
    seen: &[(NpcId, NpcSighting)],
) {
    let now = Instant::now();
    if sync.last_host_send.is_some_and(|sent| {
        now.saturating_duration_since(sent) < Duration::from_millis(NPC_SEND_INTERVAL_MS)
    }) {
        return;
    }
    sync.last_host_send = Some(now);
    let states: Vec<NpcState> = seen
        .iter()
        .filter_map(|(id, sighting)| npc_state(*id, sighting))
        .collect();
    sync.tick = sync.tick.wrapping_add(1);
    let players = [
        local
            .game
            .position()
            .map_or(remote.state.position, |at| [at.x, at.y, at.z]),
        remote.state.position,
    ];
    for part in split_npc_snapshot(sync.tick, local.phase, states, &players) {
        net.send_relayed(Message::NpcSnapshot(part));
    }
}

fn note_partner_talks(sync: &mut NpcSync, partner: &BTreeMap<NpcId, NpcState>) {
    let now: BTreeSet<NpcId> = partner.keys().copied().collect();
    for id in now.difference(&sync.partner_talking) {
        log_line(&format!(
            "npcs: the other player talks to {}, held here",
            id_text(*id)
        ));
    }
    for id in sync.partner_talking.difference(&now) {
        log_line(&format!(
            "npcs: the other player's talk with {} ended",
            id_text(*id)
        ));
    }
    sync.partner_talking = now;
}

fn apply_control(
    sync: &mut NpcSync,
    local: &LocalView<'_>,
    now_ms: u64,
    is_host: bool,
    seen: &[(NpcId, NpcSighting)],
    partner: &BTreeMap<NpcId, NpcState>,
    partner_name: &str,
) {
    let host_known = !is_host && sync.mirror.phase() == Some(local.phase);
    let samples: BTreeMap<NpcId, MirroredNpc> = if host_known {
        sync.mirror
            .sample(now_ms, NPC_MIRROR_DELAY_MS)
            .into_iter()
            .map(|npc| (npc.id, npc))
            .collect()
    } else {
        BTreeMap::new()
    };
    let mut report = NpcReport {
        local: seen.len(),
        ..NpcReport::default()
    };
    let mut held = HashSet::new();
    let mut talk_held = HashSet::new();
    let mut hidden_now = HashSet::new();
    let mut memory = InProcessMemory::new(local.build);
    let mut for_planner = Vec::new();
    for (id, sighting) in seen {
        if sighting.motion.is_some() {
            report.with_motion += 1;
        }
        let partner_talk = partner.get(id);
        match npc_control(
            is_host,
            sighting.event,
            partner_talk.is_some(),
            samples.contains_key(id),
        ) {
            NpcControl::OwnLogic => {
                report.own_logic += 1;
                sync.last_started.remove(id);
            }
            NpcControl::PartnerTalk => {
                let Some(talk) = partner_talk else {
                    continue;
                };
                report.partner_talk += 1;
                held.insert(sighting.actor);
                talk_held.insert(sighting.actor);
                let pose = Pose {
                    position: talk.position,
                    yaw_radians: talk.yaw_radians,
                    motion: talk.motion,
                };
                if drive(sync, local, &mut memory, sighting, *id, &pose) {
                    report.driven += 1;
                }
            }
            NpcControl::Host | NpcControl::Unmatched => for_planner.push(*id),
        }
    }
    if !is_host {
        let by_id: BTreeMap<NpcId, &NpcSighting> =
            seen.iter().map(|(id, sighting)| (*id, sighting)).collect();
        let remote: Vec<MirroredNpc> = samples.values().copied().collect();
        let host_current = host_known && sync.mirror.is_current(now_ms);
        let plan = sync
            .planner
            .plan(now_ms, &for_planner, &remote, host_current);
        for host in &plan.drive {
            let Some(sighting) = by_id.get(&host.id) else {
                continue;
            };
            held.insert(sighting.actor);
            if !host.visible {
                hide(sync, &mut memory, sighting);
                hidden_now.insert(sighting.actor);
                continue;
            }
            let pose = Pose {
                position: host.position,
                yaw_radians: host.yaw_radians,
                motion: host.motion,
            };
            if drive(sync, local, &mut memory, sighting, host.id, &pose) {
                report.driven += 1;
            }
        }
        for id in &plan.hide {
            if let Some(sighting) = by_id.get(id) {
                held.insert(sighting.actor);
                hide(sync, &mut memory, sighting);
                hidden_now.insert(sighting.actor);
            }
        }
        for id in &plan.newly_hidden {
            log_line(&format!(
                "npcs: hid {} (the host has no such NPC)",
                id_text(*id)
            ));
        }
        for id in &plan.newly_missing {
            log_line(&format!(
                "npcs: host NPC {} has no match here, not shown",
                id_text(*id)
            ));
        }
        report.hidden = plan.hide.len();
        report.missing_here = plan.missing_here.len();
    }
    let shown: Vec<(u64, i32)> = sync
        .hidden_by_us
        .iter()
        .filter(|(actor, _)| !hidden_now.contains(*actor))
        .map(|(actor, serial)| (*actor, *serial))
        .collect();
    for (actor, serial) in shown {
        sync.hidden_by_us.remove(&actor);
        show_again(&mut memory, actor, serial);
    }
    set_held(held);
    set_partner_talks(talk_held, partner_name);
    if sync.last_report != Some(report) {
        log_line(&format!(
            "npcs: phase {} local {} motion readable {} own logic {} partner talk {} driven {} hidden {} missing here {}",
            local.phase,
            report.local,
            report.with_motion,
            report.own_logic,
            report.partner_talk,
            report.driven,
            report.hidden,
            report.missing_here
        ));
        sync.last_report = Some(report);
    }
}

fn drive(
    sync: &mut NpcSync,
    local: &LocalView<'_>,
    memory: &mut InProcessMemory,
    sighting: &NpcSighting,
    id: NpcId,
    pose: &Pose,
) -> bool {
    let Ok(block) = memory
        .read_array::<ACTOR_TRANSFORM_BLOCK_BYTES>(sighting.actor + ACTOR_TRANSFORM_BLOCK_OFFSET)
    else {
        return false;
    };
    let matrix = shadow_matrix(&block, pose.position, pose.yaw_radians);
    if !local.calls.set_transform(memory, sighting.actor, &matrix) {
        return false;
    }
    if let Some(motion) = pose.motion {
        follow_motion(sync, local, memory, sighting.actor, id, motion);
    }
    true
}

fn follow_motion(
    sync: &mut NpcSync,
    local: &LocalView<'_>,
    memory: &mut InProcessMemory,
    actor: u64,
    id: NpcId,
    target: MotionState,
) {
    let index = i32::from(target.index);
    if !npc_motion_records_readable(memory, actor) {
        if sync.refused_motion_actors.insert(actor) {
            log_line(&format!(
                "npcs: {} has a playing motion record that cannot be read, its motion is left alone (host motion {index})",
                id_text(id)
            ));
        }
        return;
    }
    sync.refused_motion_actors.remove(&actor);
    let layer = read_npc_body_layer(memory, actor).ok().flatten();
    let last_started = sync.last_started.get(&id).copied();
    // A readable layer playing something else always restarts the target, even after our start.
    let step = match layer {
        Some(layer) if layer.motion.index != index => MotionStep::Start {
            blend: MOTION_BLEND_SECONDS,
        },
        _ => follow_step(layer.map(|layer| layer.motion), last_started, index),
    };
    match step {
        MotionStep::Start { blend } => {
            local.calls.play_body_motion(
                actor,
                index,
                blend,
                target.looping,
                target.speed,
                target.frame,
            );
            if last_started.is_none() {
                log_line(&format!(
                    "npcs: {} first motion {index} from frame {:.1} (was {})",
                    id_text(id),
                    target.frame,
                    layer.map_or(-1, |layer| layer.motion.index)
                ));
            }
            sync.last_started.insert(id, index);
        }
        MotionStep::Keep => {
            if let Some(layer) = layer.filter(|layer| layer.motion.index == index)
                && let Some(frame) = frame_to_seek(
                    layer.motion.frame,
                    target.frame,
                    layer.frame_count,
                    layer.motion.looping,
                )
            {
                let _ = seek_layer_frame(memory, &layer, frame);
            }
        }
    }
}

fn hide(sync: &mut NpcSync, memory: &mut InProcessMemory, sighting: &NpcSighting) {
    if sync.hidden_by_us.contains_key(&sighting.actor) || !sighting.visible {
        return;
    }
    if memory
        .write_u8(sighting.actor + ACTOR_DRAWN_FLAG_OFFSET, 0)
        .is_ok()
    {
        sync.hidden_by_us
            .insert(sighting.actor, sighting.spawn_serial);
    }
}

fn show_again(memory: &mut InProcessMemory, actor: u64, serial: i32) {
    // Only an actor that is still the one we hid gets its flag back; a freed slot is left alone.
    let still_ours = actor_is_live(memory, actor).unwrap_or(false)
        && memory
            .read_i32(actor + ACTOR_SPAWN_SERIAL_OFFSET)
            .is_ok_and(|now| now == serial);
    if still_ours {
        let _ = memory.write_u8(actor + ACTOR_DRAWN_FLAG_OFFSET, 1);
    }
}

fn id_text(id: NpcId) -> String {
    format!("{}:{}:{}", id.group, id.class, id.ordinal)
}
