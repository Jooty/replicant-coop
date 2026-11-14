use std::collections::{BTreeMap, HashSet, VecDeque};
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::Instant;

use replicant_game::enemies::{
    CountLog, ENEMY_ID_LIST_INTERVAL_MS, ENEMY_MIRROR_DELAY_MS, ENEMY_ROUND_CAPACITY, EnemyLog,
    EnemyMirror, EnemyRoster, EnemySighting, FIGHT_SPAWN_FILL_IN_GRACE_MS, FightSpawnFillIn,
    HostFightEnemy, IntervalClock, ListDiffWatch, MirroredEnemy, RemovalCause, RosterUpdate,
    WaitingFightSpawn, enemy_state, fight_spawn_fill_in_gave_up_text,
    fight_spawn_fill_in_snap_off_text, fight_spawn_filled_in_text, match_enemies,
    missing_host_spawns, partner_actors, plan_fight_bindings, plan_takeovers, relayed_spawn_text,
    scaled_matrix, scan_enemies, shadow_matrix, split_id_list, split_snapshot,
    stalled_copy_time_scale_fix,
};
use replicant_game::npcs::{
    EventSceneClock, NPC_EVENT_SCENE_SETTLE_MS, npc_motion_records_readable, read_enemy_body_layer,
    read_npc_hold_blocker,
};
use replicant_game::offsets::{
    ACTOR_SPAWN_SERIAL_OFFSET, ACTOR_TIME_SCALE_OFFSET, ACTOR_TRANSFORM_BLOCK_BYTES,
    ACTOR_TRANSFORM_BLOCK_OFFSET,
};
use replicant_game::puppet::{
    LayerMotion, MOTION_BLEND_SECONDS, MotionStep, follow_step, frame_to_seek, read_puppet_layer,
    seek_layer_frame,
};
use replicant_game::{GameMemory, PhaseEntry, Replicant};
use replicant_proto::{
    EnemyCreation, EnemyGone, EnemyGoneReason, EnemyHit, EnemyId, EnemySpawned, Message,
    MotionState, SessionOptions,
};

use crate::attack_gauge;
use crate::boss_messages::{is_boss, observe_boss_fight};
use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::enemies::boss_bar_probe::log_boss_bar_state;
use crate::enemies::boss_motion_probe::log_boss_motion_layers;
use crate::enemies::ca_gauge_probe::log_ca_gauge_entries;
use crate::enemies::combat::CombatState;
use crate::enemies::combat_view::clear_view;
use crate::enemies::companion::{self, CompanionShare};
use crate::enemies::companion_probe::log_live_companions;
use crate::enemies::game_calls::EnemyCalls;
use crate::enemies::hold_hook::{release_held, set_held, set_stopped_dead, take_own_steps_cut};
use crate::enemies::spawn_hook::take_creation;
use crate::enemies::status::{
    EnemySyncRole, EnemySyncStatus, enemy_id_text, publish_status, snap_enabled,
};
use crate::enemies::{
    MAX_RECENT_ENEMY_EVENTS, MOTION_FRAME_MOVED, MOTION_RESTART_INTERVAL_MS, MOTION_STALL_MS,
    SECOND_MOTION_LAYER,
};
use crate::enemies::{guest_combat, host_combat};
use crate::memory::InProcessMemory;
use crate::net::{self, NetThread, RemotePlayer, SEND_INTERVAL};

static SYNC: LazyLock<Mutex<EnemySync>> = LazyLock::new(|| Mutex::new(EnemySync::default()));

#[derive(Default)]
struct EnemySync {
    role: Option<EnemySyncRole>,
    roster: EnemyRoster,
    mirror: EnemyMirror,
    epoch: Option<Instant>,
    last_host_send: Option<Instant>,
    tick: u32,
    events: VecDeque<String>,
    combat: CombatState,
    log: EnemyLog,
    diff_watch: ListDiffWatch,
    count_log: CountLog,
    id_list_clock: IntervalClock,
    scene_clock: EventSceneClock,
    logged_settling: bool,
    fight_fill_in: FightSpawnFillIn,
    companions: CompanionShare,
}

pub(super) struct LocalView<'a> {
    pub(super) build: &'a VerifiedBuild,
    pub(super) game: &'a Replicant<InProcessMemory>,
    pub(super) calls: EnemyCalls,
    pub(super) phase: u16,
}

impl EnemySync {
    fn go_idle(&mut self, build: &VerifiedBuild, reason: &'static str) {
        host_combat::restore_enemy_health(&mut self.combat, build);
        guest_combat::show_hidden_copies(&mut self.combat, &mut InProcessMemory::new(build));
        self.role = None;
        self.roster.reset();
        self.log.reset();
        self.diff_watch.reset();
        self.count_log.reset();
        self.id_list_clock.reset();
        self.mirror.clear();
        self.last_host_send = None;
        self.release_combat();
        let mut status = EnemySyncStatus::idle(reason);
        status.recent_events.clone_from(&self.events);
        status.combat = self.combat.counts.with_hook_counts();
        publish_status(status);
    }

    fn take_role(&mut self, build: &VerifiedBuild, role: EnemySyncRole) {
        if self.role != Some(role) {
            host_combat::restore_enemy_health(&mut self.combat, build);
            guest_combat::show_hidden_copies(&mut self.combat, &mut InProcessMemory::new(build));
            self.roster.reset();
            self.log.reset();
            self.diff_watch.reset();
            self.count_log.reset();
            self.id_list_clock.reset();
            self.mirror.clear();
            self.last_host_send = None;
            self.release_combat();
            self.role = Some(role);
        }
    }

    fn release_combat(&mut self) {
        self.companions = CompanionShare::default();
        self.combat.reset();
        clear_view();
        set_held(HashSet::new());
        set_stopped_dead(HashSet::new());
        let _ = take_own_steps_cut();
    }

    fn now_ms(&mut self) -> u64 {
        let epoch = *self.epoch.get_or_insert_with(Instant::now);
        u64::try_from(epoch.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn note(&mut self, event: String) {
        log_line(&format!("enemies: {event}"));
        if self.events.len() >= MAX_RECENT_ENEMY_EVENTS {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    fn status(&self) -> EnemySyncStatus {
        EnemySyncStatus {
            idle_reason: None,
            role: self.role,
            recent_events: self.events.clone(),
            combat: self.combat.counts.with_hook_counts(),
            ..EnemySyncStatus::default()
        }
    }
}

pub fn tick_enemy_sync(build: &VerifiedBuild) {
    let mut sync = SYNC.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(net) = net::handle() else {
        sync.go_idle(build, "co-op is off");
        return;
    };
    let relayed = net.take_relayed(|message| {
        matches!(
            message,
            Message::EnemySnapshot(_)
                | Message::EnemyIdList(_)
                | Message::EnemySpawned(_)
                | Message::EnemyCreation(_)
                | Message::EnemyGone(_)
                | Message::EnemyHit(_)
                | Message::PlayerHit(_)
                | Message::EnemyKilled(_)
                | Message::EnemyDrop(_)
                | Message::EnemyWordDrop(_)
                | Message::EnemyBodyGone(_)
        )
    });
    let view = net.view();
    let options = view.options;
    let Some(remote) = view.partner_state().cloned() else {
        sync.go_idle(build, "no other player is connected");
        return;
    };
    let role = if remote.slot == 1 {
        EnemySyncRole::Host
    } else {
        EnemySyncRole::Guest
    };
    sync.take_role(build, role);
    let game = Replicant::new(InProcessMemory::new(build));
    let Some(phase) = local_phase(&game) else {
        sync.go_idle(build, "your own phase is not readable (loading?)");
        return;
    };
    let local = LocalView {
        build,
        game: &game,
        calls: EnemyCalls::new(build),
        phase,
    };
    match role {
        EnemySyncRole::Host => host_step(&mut sync, net, &local, &remote, &relayed, options),
        EnemySyncRole::Guest => guest_step(&mut sync, &local, relayed),
    }
}

fn local_phase(game: &Replicant<InProcessMemory>) -> Option<u16> {
    let name = game.current_phase().ok()?;
    u16::try_from(PhaseEntry::index_of(&name)?).ok()
}

fn send_creation(net: &NetThread, phase: u16, id: EnemyId, seen: &[(EnemyId, EnemySighting)]) {
    let Some((_, sighting)) = seen.iter().find(|(seen_id, _)| *seen_id == id) else {
        return;
    };
    let Some(creation) = take_creation(sighting.actor, sighting.spawn_serial) else {
        return;
    };
    net.send_relayed(Message::EnemyCreation(EnemyCreation {
        phase,
        id,
        actor_name: creation.actor_name,
        spec_slot: creation.spec_slot,
        spawn_flag: creation.spawn_flag,
        attach_flag: creation.attach_flag,
        scale: creation.scale,
        spec: creation.spec,
    }));
}

fn create_missing_copies(
    sync: &mut EnemySync,
    local: &LocalView<'_>,
    now_ms: u64,
    seen: &[(EnemyId, EnemySighting)],
    samples: &BTreeMap<EnemyId, MirroredEnemy>,
) {
    let host_live: Vec<EnemyId> = samples.keys().copied().collect();
    let own: HashSet<EnemyId> = seen.iter().map(|(id, _)| *id).collect();
    let ended = ended_or_dying_ids(sync);
    let creation_known = copyable_host_ids(sync, local.phase);
    let missing = missing_host_spawns(
        &host_live,
        &own,
        &sync.combat.created,
        &ended,
        &creation_known,
    );
    for id in sync.combat.creation_watch.due(now_ms, &missing) {
        let Some(host) = samples.get(&id) else {
            continue;
        };
        if let Some(name) = put_host_copy(sync, local, id, host) {
            sync.note(format!(
                "created {} here from the host's spawn (name {name})",
                enemy_id_text(id)
            ));
        }
    }
}

fn ended_or_dying_ids(sync: &EnemySync) -> HashSet<EnemyId> {
    let mut ended = sync.combat.ended_ids.clone();
    ended.extend(
        sync.combat
            .pending_kills
            .iter()
            .map(|(killed, _)| killed.id),
    );
    ended
}

fn copyable_host_ids(sync: &EnemySync, phase: u16) -> HashSet<EnemyId> {
    sync.combat
        .creations
        .iter()
        .filter(|(id, creation)| creation.phase == phase && !is_boss(**id))
        .map(|(id, _)| *id)
        .collect()
}

fn put_host_copy(
    sync: &mut EnemySync,
    local: &LocalView<'_>,
    id: EnemyId,
    host: &MirroredEnemy,
) -> Option<String> {
    let creation = sync.combat.creations.get(&id)?;
    let matrix = scaled_matrix(host.position, host.yaw_radians, creation.scale);
    sync.combat.created.insert(id);
    let Some(actor) = local.calls.put_enemy_actor(creation, id.group, &matrix) else {
        let name = creation.actor_name.clone();
        sync.note(format!(
            "cause: the host spawned {} (name {name}) and the game would not create a copy here",
            enemy_id_text(id)
        ));
        return None;
    };
    let name = creation.actor_name.clone();
    let serial = InProcessMemory::new(local.build)
        .read_i32(actor + ACTOR_SPAWN_SERIAL_OFFSET)
        .unwrap_or_default();
    sync.roster.claim(
        actor,
        serial,
        id,
        i32::try_from(host.hp).unwrap_or(i32::MAX),
    );
    Some(name)
}

fn scan(local: &LocalView<'_>) -> Vec<EnemySighting> {
    let memory = local.game.memory();
    let calls = local.calls;
    scan_enemies(
        memory,
        local.game.playable_actor().ok(),
        &partner_actors(memory),
        |group, index| calls.actor_in_group(group, index),
        |actor| calls.hp_of(memory, actor),
    )
    .unwrap_or_default()
}

fn host_step(
    sync: &mut EnemySync,
    net: &NetThread,
    local: &LocalView<'_>,
    remote: &RemotePlayer,
    relayed: &[Message],
    options: SessionOptions,
) {
    let now_ms = sync.now_ms();
    let hits: Vec<EnemyHit> = relayed
        .iter()
        .filter_map(|message| match message {
            Message::EnemyHit(hit) => Some(*hit),
            _ => None,
        })
        .collect();
    host_combat::apply_guest_hits(&mut sync.combat, local, now_ms, &hits);
    let now = Instant::now();
    if sync
        .last_host_send
        .is_some_and(|sent| now.saturating_duration_since(sent) < SEND_INTERVAL)
    {
        return;
    }
    sync.last_host_send = Some(now);
    let sightings = scan(local);
    let mut update = sync.roster.observe(local.phase, &sightings);
    note_last_phase_leftovers(sync, &update);
    observe_boss_fight(local.phase, &update.seen);
    log_boss_motion_layers(local.build, "host", &update.seen);
    log_live_companions(local.build, "host");
    log_ca_gauge_entries(local.build, &update.seen);
    attack_gauge::tick_host(local.build, net, local.phase, &update.seen);
    log_boss_bar_state(local.build, &update.seen);
    let guest_killed = sync.combat.guest_kills.clone();
    host_combat::after_scan(
        &mut sync.combat,
        local,
        net,
        now_ms,
        &mut update,
        remote,
        options,
    );
    log_host_changes(sync, &update, &guest_killed);
    let in_scene = read_npc_hold_blocker(local.game.memory())
        .ok()
        .flatten()
        .is_some();
    sync.scene_clock.observe(now_ms, in_scene);
    let companions = companion::host_companions(
        &mut sync.companions,
        &mut sync.combat,
        local,
        in_scene,
        sync.scene_clock.settling(now_ms),
    );
    let mut status = sync.status();
    status.local_count = update.seen.len();
    status.host_total = Some(u16::try_from(update.seen.len()).unwrap_or(u16::MAX));
    if remote.state.phase != local.phase {
        status.idle_reason = Some("the other player is in another phase, nothing is sent");
        publish_status(status);
        return;
    }
    let states: BTreeMap<EnemyId, _> = update
        .seen
        .iter()
        .map(|(id, sighting)| (*id, sighting))
        .chain(
            companions
                .iter()
                .map(|companion| (companion.id, &companion.sighting)),
        )
        .filter_map(|(id, sighting)| Some((id, enemy_state(id, sighting)?)))
        .collect();
    for id in &update.spawned {
        if let Some(enemy) = states.get(id) {
            net.send_relayed(Message::EnemySpawned(EnemySpawned {
                phase: local.phase,
                enemy: *enemy,
            }));
        }
    }
    for id in &update.spawned {
        send_creation(net, local.phase, *id, &update.seen);
    }
    for (id, reason) in &update.gone {
        net.send_relayed(Message::EnemyGone(EnemyGone {
            phase: local.phase,
            id: *id,
            reason: *reason,
        }));
    }
    sync.tick = sync.tick.wrapping_add(1);
    let players = [
        local
            .game
            .position()
            .map_or(remote.state.position, |at| [at.x, at.y, at.z]),
        remote.state.position,
    ];
    if let Some(text) = sync.count_log.host_line(states.len(), ENEMY_ROUND_CAPACITY) {
        sync.note(text);
    }
    if sync.id_list_clock.due(now_ms, ENEMY_ID_LIST_INTERVAL_MS) {
        let ids: Vec<EnemyId> = states.keys().copied().collect();
        for list in split_id_list(sync.tick, local.phase, ids) {
            net.send_relayed(Message::EnemyIdList(list));
        }
    }
    let parts = split_snapshot(
        sync.tick,
        local.phase,
        states.into_values().collect(),
        &players,
    );
    status.sent_parts = parts.len();
    for part in parts {
        net.send_relayed(Message::EnemySnapshot(part));
    }
    publish_status(status);
}

fn guest_step(sync: &mut EnemySync, local: &LocalView<'_>, relayed: Vec<Message>) {
    let now_ms = sync.now_ms();
    for message in relayed {
        receive(sync, now_ms, &message);
        guest_combat::receive(&mut sync.combat, now_ms, &message);
    }
    sync.mirror.prune(now_ms);
    let sightings = scan(local);
    let mut update = sync.roster.observe(local.phase, &sightings);
    note_last_phase_leftovers(sync, &update);
    observe_boss_fight(local.phase, &update.seen);
    log_boss_motion_layers(local.build, "guest", &update.seen);
    log_live_companions(local.build, "guest");
    log_ca_gauge_entries(local.build, &update.seen);
    attack_gauge::tick_guest(local.build, local.phase, &update.seen);
    log_boss_bar_state(local.build, &update.seen);
    let mut samples: BTreeMap<EnemyId, MirroredEnemy> = sync
        .mirror
        .sample(now_ms, ENEMY_MIRROR_DELAY_MS)
        .into_iter()
        .map(|enemy| (enemy.id, enemy))
        .collect();
    // Companion ids sort after every script group's, so this takes exactly the companions.
    let host_companions = samples.split_off(&EnemyId::companion(0));
    let inside_fight_window = crate::fight_sync::covers_phase(local.phase);
    if inside_fight_window {
        bind_fight_spawns(sync, local, &mut update, &samples);
    } else {
        sync.roster.stop_all_waiting();
        take_over_copies(sync, local, &mut update, &samples);
        adopt_host_ids(sync, local.phase, &mut update, &samples);
    }
    log_guest_changes(sync, &update, &samples);
    let mut status = sync.status();
    status.local_count = update.seen.len();
    status.host_total = sync.mirror.host_total();
    status.mirrored = sync.mirror.len();
    let in_scene = read_npc_hold_blocker(local.game.memory())
        .ok()
        .flatten()
        .is_some();
    sync.scene_clock.observe(now_ms, in_scene);
    if sync.scene_clock.settling(now_ms) {
        if !sync.logged_settling {
            sync.logged_settling = true;
            sync.note(format!(
                "event scene just ended, copies are neither held nor driven for {NPC_EVENT_SCENE_SETTLE_MS} ms while the game cleans up the scene"
            ));
        }
        status.idle_reason = Some("an event scene just ended, the game is cleaning it up");
        guest_combat::show_hidden_copies(&mut sync.combat, &mut InProcessMemory::new(local.build));
        sync.release_combat();
        publish_status(status);
        return;
    }
    sync.logged_settling = false;
    if sync.mirror.phase() != Some(local.phase) {
        status.idle_reason = Some("no enemy snapshot from the host for your phase yet");
        guest_combat::show_hidden_copies(&mut sync.combat, &mut InProcessMemory::new(local.build));
        sync.release_combat();
        publish_status(status);
        return;
    }
    let here: Vec<(EnemyId, [f32; 3])> = update
        .seen
        .iter()
        .map(|(id, sighting)| (*id, sighting.position))
        .collect();
    let there: Vec<(EnemyId, [f32; 3])> = samples
        .values()
        .map(|enemy| (enemy.id, enemy.position))
        .collect();
    let report = match_enemies(&here, &there);
    status.matched = report.matched.len();
    status.worst_error = report.worst_error();
    status.unmatched_local = report.unmatched_local;
    status.unmatched_remote = report.unmatched_remote;
    let authority = snap_enabled();
    let host_view = sync.mirror.host_view(now_ms);
    if let Some(total) = sync.mirror.host_total()
        && let Some(round) = sync.mirror.latest_complete_round()
        && let Some(text) = sync.count_log.guest_line(
            now_ms,
            total,
            round.round,
            sync.mirror.len(),
            update.seen.len(),
        )
    {
        sync.note(text);
    }
    if !inside_fight_window {
        sync.fight_fill_in.clear();
        if authority {
            create_missing_copies(sync, local, now_ms, &update.seen, &samples);
        }
    } else {
        fill_in_fight_spawns(sync, local, now_ms, authority, &update.seen, &samples);
    }
    guest_combat::step(
        &mut sync.combat,
        local,
        now_ms,
        &update.seen,
        &samples,
        authority,
        &host_view,
    );
    let driven_companions = companion::guest_companions(
        &mut sync.companions,
        &mut sync.combat,
        local,
        &host_companions,
        authority,
        in_scene,
    );
    let diff = guest_combat::list_diff(&sync.combat, &update.seen, &samples, &host_view);
    if let Some(text) = sync.diff_watch.observe(now_ms, diff) {
        sync.note(text);
    }
    if authority {
        for (id, sighting) in &update.seen {
            let Some(host) = samples.get(id) else {
                continue;
            };
            if sync.combat.killed_copies.contains(&sighting.actor)
                || sync.combat.own_script_villagers.contains(&sighting.actor)
            {
                continue;
            }
            if let Some(motion) = host.motion {
                follow_motion(&mut sync.combat, local, sighting.actor, *id, motion, now_ms);
            }
            if snap(local, sighting.actor, host) {
                status.snapped += 1;
            } else {
                status.snap_failures += 1;
            }
        }
    }
    for driven in &driven_companions {
        if let Some(motion) = driven.host.motion {
            follow_motion(
                &mut sync.combat,
                local,
                driven.actor,
                driven.id,
                motion,
                now_ms,
            );
        }
        if snap(local, driven.actor, &driven.host) {
            status.snapped += 1;
        } else {
            status.snap_failures += 1;
        }
    }
    status.combat = sync.combat.counts.with_hook_counts();
    publish_status(status);
}

fn follow_motion(
    combat: &mut CombatState,
    local: &LocalView<'_>,
    actor: u64,
    id: EnemyId,
    target: MotionState,
    now_ms: u64,
) {
    let mut memory = InProcessMemory::new(local.build);
    if !npc_motion_records_readable(&memory, actor) {
        if combat.unreadable_motion_actors.insert(actor) {
            log_line(&format!(
                "enemies: {} has a playing motion record that cannot be read, its motion is left alone",
                enemy_id_text(id)
            ));
        }
        return;
    }
    combat.unreadable_motion_actors.remove(&actor);
    let index = i32::from(target.index);
    let layer = read_enemy_body_layer(&memory, actor).ok().flatten();
    let started = combat.last_started.get(&id).copied();
    let watch = combat.motion_watch.entry(id).or_default();
    let stalled = match layer {
        None => true,
        Some(layer) if layer.motion.index == index && target.speed != 0.0 => {
            if (layer.motion.frame - watch.frame).abs() > MOTION_FRAME_MOVED {
                watch.frame = layer.motion.frame;
                watch.since_ms = now_ms;
                false
            } else {
                now_ms.saturating_sub(watch.since_ms) >= MOTION_STALL_MS
            }
        }
        Some(_) => false,
    };
    if !stalled {
        watch.reported = false;
    } else if !watch.reported {
        watch.reported = true;
        log_motion_stall(
            &memory,
            actor,
            id,
            index,
            target,
            layer.map(|layer| layer.motion),
        );
    }
    let restart_due = stalled
        && watch
            .restarted_at_ms
            .is_none_or(|at| now_ms.saturating_sub(at) >= MOTION_RESTART_INTERVAL_MS);
    // A readable layer playing something else always restarts the target, even after our start.
    let step = match layer {
        _ if restart_due => MotionStep::Start {
            blend: MOTION_BLEND_SECONDS,
        },
        Some(layer) if layer.motion.index != index => MotionStep::Start {
            blend: MOTION_BLEND_SECONDS,
        },
        _ => follow_step(layer.map(|layer| layer.motion), started, index),
    };
    if restart_due {
        watch.restarted_at_ms = Some(now_ms);
        watch.since_ms = now_ms;
        reset_stalled_time_scale(&mut memory, actor, id);
    }
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
            combat.last_started.insert(id, index);
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
                let _ = seek_layer_frame(&mut memory, &layer, frame);
            }
        }
    }
}

fn reset_stalled_time_scale(memory: &mut InProcessMemory, actor: u64, id: EnemyId) {
    let Ok(current) = memory.read_f32(actor + ACTOR_TIME_SCALE_OFFSET) else {
        return;
    };
    let Some(target) = stalled_copy_time_scale_fix(current) else {
        return;
    };
    if memory
        .write_f32(actor + ACTOR_TIME_SCALE_OFFSET, target)
        .is_ok()
    {
        log_line(&format!(
            "combat: your stalled copy of {} had time scale {current:.2}; put back to {target:.2}",
            enemy_id_text(id)
        ));
    }
}

fn log_motion_stall(
    memory: &InProcessMemory,
    actor: u64,
    id: EnemyId,
    index: i32,
    target: MotionState,
    body: Option<LayerMotion>,
) {
    let describe = |motion: Option<LayerMotion>| {
        motion.map_or_else(
            || "unreadable".to_owned(),
            |motion| {
                format!(
                    "motion {} frame {:.1} speed {:.2}",
                    motion.index, motion.frame, motion.speed
                )
            },
        )
    };
    let second = read_puppet_layer(memory, actor, SECOND_MOTION_LAYER)
        .ok()
        .flatten()
        .map(|layer| layer.motion);
    let time_scale = memory
        .read_f32(actor + ACTOR_TIME_SCALE_OFFSET)
        .map_or_else(|_| "unreadable".to_owned(), |scale| format!("{scale:.2}"));
    log_line(&format!(
        "combat: your copy of {} stopped animating (host motion {index} frame {:.1} speed {:.2}; body layer {}; layer 1 {}; time scale {time_scale}); restarting the host's motion",
        enemy_id_text(id),
        target.frame,
        target.speed,
        describe(body),
        describe(second)
    ));
}

fn receive(sync: &mut EnemySync, now_ms: u64, message: &Message) {
    match message {
        Message::EnemySnapshot(snapshot) => sync.mirror.receive_snapshot(now_ms, snapshot),
        Message::EnemyIdList(list) => sync.mirror.receive_id_list(now_ms, list),
        Message::EnemySpawned(spawned) => {
            sync.mirror.receive_spawned(now_ms, spawned);
            sync.note(relayed_spawn_text(&spawned.enemy));
        }
        Message::EnemyGone(gone) => {
            sync.mirror.receive_gone(gone);
            sync.note(format!(
                "host {} {}",
                gone_text(gone.reason),
                enemy_id_text(gone.id)
            ));
        }
        _ => {}
    }
}

fn snap(local: &LocalView<'_>, actor: u64, host: &MirroredEnemy) -> bool {
    let memory = local.game.memory();
    let Ok(block) =
        memory.read_array::<ACTOR_TRANSFORM_BLOCK_BYTES>(actor + ACTOR_TRANSFORM_BLOCK_OFFSET)
    else {
        return false;
    };
    let matrix = shadow_matrix(&block, host.position, host.yaw_radians);
    if !local.calls.set_transform(memory, actor, &matrix) {
        return false;
    }
    let Ok(host_hp) = i32::try_from(host.hp) else {
        return true;
    };
    // A host HP of zero means it is dying there; its death arrives as an event instead.
    if host_hp > 0
        && let Some((hp, _)) = local.calls.hp_of(memory, actor)
        && hp != host_hp
    {
        return local.calls.set_hp(memory, actor, host_hp);
    }
    true
}

fn note_last_phase_leftovers(sync: &mut EnemySync, update: &RosterUpdate) {
    if update.last_phase_leftovers_found > 0 {
        sync.note(format!(
            "the phase changed while the game still holds {} enemies of the last phase, they get no id in this phase",
            update.last_phase_leftovers_found
        ));
    }
    if update.last_phase_leftovers_gone > 0 {
        sync.note(format!(
            "{} enemies of the last phase are gone now",
            update.last_phase_leftovers_gone
        ));
    }
}

fn log_host_changes(sync: &mut EnemySync, update: &RosterUpdate, guest_killed: &HashSet<EnemyId>) {
    for (id, reason) in &update.gone {
        let cause = match reason {
            EnemyGoneReason::Killed if guest_killed.contains(id) => RemovalCause::KilledByGuest,
            EnemyGoneReason::Killed => RemovalCause::KilledByHost,
            EnemyGoneReason::Removed => RemovalCause::GameRemoval,
        };
        sync.log.note_cause(*id, cause);
    }
    let lines = sync
        .log
        .observe(update, |_| "host spawn, sent to the guest".to_owned());
    for line in lines {
        sync.note(line);
    }
}

fn take_over_copies(
    sync: &mut EnemySync,
    local: &LocalView<'_>,
    update: &mut RosterUpdate,
    samples: &BTreeMap<EnemyId, MirroredEnemy>,
) {
    if update.spawned.is_empty()
        || sync.mirror.phase() != Some(local.phase)
        || sync.combat.created.is_empty()
    {
        return;
    }
    let host_live: Vec<EnemyId> = samples
        .keys()
        .filter(|id| !sync.combat.ended_ids.contains(id))
        .copied()
        .collect();
    let own_spawns: Vec<(EnemyId, EnemySighting)> = update
        .seen
        .iter()
        .filter(|(id, _)| update.spawned.contains(id))
        .copied()
        .collect();
    let copies: Vec<(EnemyId, EnemySighting)> = update
        .seen
        .iter()
        .filter(|(id, _)| sync.combat.created.contains(id) && !update.spawned.contains(id))
        .copied()
        .collect();
    let plan = plan_takeovers(&own_spawns, &copies, &host_live, &sync.combat.taken_over);
    let mut adoptions = Vec::new();
    for (own, copy) in plan {
        if !destroy_taken_over_copy(sync, local, update, own, copy) {
            continue;
        }
        adoptions.push((own, copy));
        sync.note(format!(
            "own spawn {} took over the host's copy (your game's id was {})",
            enemy_id_text(copy),
            enemy_id_text(own)
        ));
    }
    update.rename(&adoptions);
}

fn destroy_taken_over_copy(
    sync: &mut EnemySync,
    local: &LocalView<'_>,
    update: &mut RosterUpdate,
    own: EnemyId,
    copy: EnemyId,
) -> bool {
    let Some(actor) = update
        .seen
        .iter()
        .find(|(id, _)| *id == copy)
        .map(|(_, sighting)| sighting.actor)
    else {
        return false;
    };
    if !sync.roster.take_over_copy(own, copy) {
        return false;
    }
    update.seen.retain(|(_, sighting)| sighting.actor != actor);
    sync.combat.killed_copies.insert(actor);
    release_held(actor);
    local.calls.destroy(actor);
    sync.combat.taken_over.insert(copy);
    true
}

fn fill_in_fight_spawns(
    sync: &mut EnemySync,
    local: &LocalView<'_>,
    now_ms: u64,
    authority: bool,
    seen: &[(EnemyId, EnemySighting)],
    samples: &BTreeMap<EnemyId, MirroredEnemy>,
) {
    let host_live: Vec<EnemyId> = samples.keys().copied().collect();
    let here: HashSet<EnemyId> = seen.iter().map(|(id, _)| *id).collect();
    let ended = ended_or_dying_ids(sync);
    let copyable = copyable_host_ids(sync, local.phase);
    let step = sync
        .fight_fill_in
        .observe(now_ms, &host_live, &here, &ended, &copyable);
    for id in step.newly_waiting {
        sync.note(format!(
            "the host's {} has no enemy here yet: inside the fight window your game's replay of the host's handler spawns it, or a copy fills in after {FIGHT_SPAWN_FILL_IN_GRACE_MS} ms",
            enemy_id_text(id)
        ));
    }
    if !authority {
        if let Some((id, _)) = step.due.first()
            && sync.fight_fill_in.first_skip_with_snap_off()
        {
            sync.note(fight_spawn_fill_in_snap_off_text(*id));
        }
        return;
    }
    for (id, waited_ms) in step.due {
        let Some(host) = samples.get(&id) else {
            continue;
        };
        // `created` also holds our own failed tries; only a copy made another way ends the wait.
        if sync.combat.created.contains(&id) && !sync.fight_fill_in.tried(id) {
            sync.fight_fill_in.mark_filled_in(id);
            continue;
        }
        if put_host_copy(sync, local, id, host).is_some() {
            sync.fight_fill_in.mark_filled_in(id);
            sync.note(fight_spawn_filled_in_text(id, waited_ms));
        } else if let Some(tries) = sync.fight_fill_in.copy_failed(id, now_ms) {
            sync.note(fight_spawn_fill_in_gave_up_text(id, tries));
        }
    }
}

fn bind_fight_spawns(
    sync: &mut EnemySync,
    local: &LocalView<'_>,
    update: &mut RosterUpdate,
    samples: &BTreeMap<EnemyId, MirroredEnemy>,
) {
    let mut waiting = Vec::new();
    for id in sync.roster.fight_waiting_spawns() {
        let Some((_, sighting)) = update.seen.iter().find(|(seen, _)| *seen == id) else {
            continue;
        };
        if is_boss(id) {
            sync.roster.stop_waiting(id);
            continue;
        }
        waiting.push(WaitingFightSpawn {
            id,
            max_hp: sighting.max_hp,
            position: sighting.position,
        });
    }
    if waiting.is_empty() {
        return;
    }
    let ended = ended_or_dying_ids(sync);
    let host_live: Vec<HostFightEnemy> = samples
        .values()
        .filter(|host| !ended.contains(&host.id))
        .map(|host| HostFightEnemy {
            id: host.id,
            max_hp: host.max_hp,
            position: host.position,
        })
        .collect();
    let waiting_ids: HashSet<EnemyId> = waiting.iter().map(|spawn| spawn.id).collect();
    let copies_here: HashSet<EnemyId> = update
        .seen
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| sync.combat.created.contains(id) && !waiting_ids.contains(id))
        .collect();
    let held_by_others: HashSet<EnemyId> = sync
        .roster
        .held_ids()
        .into_iter()
        .filter(|id| !waiting_ids.contains(id) && !copies_here.contains(id))
        .collect();
    let plan = plan_fight_bindings(
        &waiting,
        &host_live,
        &ended,
        &held_by_others,
        &copies_here,
        &sync.combat.taken_over,
    );
    for id in &plan.kept {
        sync.roster.stop_waiting(*id);
    }
    let mut adoptions = Vec::new();
    for (own, copy) in plan.takeovers {
        if !destroy_taken_over_copy(sync, local, update, own, copy) {
            continue;
        }
        sync.roster.stop_waiting(copy);
        adoptions.push((own, copy));
        sync.note(format!(
            "your game's spawn {} inside the fight window took over the host's copy {} (same group and max hp, nearest)",
            enemy_id_text(own),
            enemy_id_text(copy)
        ));
    }
    update.rename(&adoptions);
    let mut moves = plan.renames.clone();
    for id in &plan.retired {
        let retired = sync.roster.retired_id(id.group);
        moves.push((*id, retired));
    }
    let made = sync.roster.reassign_ids(&moves);
    update.rename(&made);
    for (own, to) in made {
        if plan.renames.contains(&(own, to)) {
            sync.roster.stop_waiting(to);
            sync.note(format!(
                "your game's spawn {} inside the fight window is the host's {} (same group and max hp, nearest)",
                enemy_id_text(own),
                enemy_id_text(to)
            ));
        } else {
            sync.note(format!(
                "your game's spawn {} inside the fight window matches no live host enemy, so it is renamed {}, an id the host never has; it waits for the host's spawn or is removed",
                enemy_id_text(own),
                enemy_id_text(to)
            ));
        }
    }
}

fn adopt_host_ids(
    sync: &mut EnemySync,
    phase: u16,
    update: &mut RosterUpdate,
    samples: &BTreeMap<EnemyId, MirroredEnemy>,
) {
    if update.spawned.is_empty() || sync.mirror.phase() != Some(phase) {
        return;
    }
    let host_live: Vec<EnemyId> = samples
        .keys()
        .filter(|id| !sync.combat.ended_ids.contains(id))
        .copied()
        .collect();
    let adoptions = sync.roster.adopt_host_ids(&update.spawned, &host_live);
    if adoptions.is_empty() {
        return;
    }
    update.rename(&adoptions);
    for (own, adopted) in adoptions {
        sync.note(format!(
            "adopted the host's id {} for your new {}, so one copy of it exists on each game",
            enemy_id_text(adopted),
            enemy_id_text(own)
        ));
    }
}

fn log_guest_changes(
    sync: &mut EnemySync,
    update: &RosterUpdate,
    samples: &BTreeMap<EnemyId, MirroredEnemy>,
) {
    for (id, cause) in std::mem::take(&mut sync.combat.removal_causes) {
        sync.log.note_cause(id, cause);
    }
    let lines = sync.log.observe(update, |id| {
        if samples.contains_key(&id) {
            "your game spawned it, the host has it".to_owned()
        } else {
            "your game spawned it, not in the host's list yet".to_owned()
        }
    });
    for line in lines {
        sync.note(line);
    }
}

fn gone_text(reason: EnemyGoneReason) -> &'static str {
    match reason {
        EnemyGoneReason::Killed => "killed",
        EnemyGoneReason::Removed => "removed",
    }
}
