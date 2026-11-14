use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::{LazyLock, Mutex, PoisonError};

use replicant_game::npcs::read_npc_body_layer;
use replicant_game::offsets::{ACTOR_MODEL_PACK_OFFSET, BODY_MOTION_LAYER, PUPPET_UNIQUE_ID_BASE};
use replicant_game::power_models::{
    PowerCastLog, PowerModelTable, power_model_is_prop, power_model_pack_name,
};
use replicant_game::puppet::{
    MotionStep, PuppetPose, RespawnGate, follow_step, frame_to_seek, seek_layer_frame,
};
use replicant_game::{GameMemory, Replicant};
use replicant_proto::{Message, MotionState, PowerModelState};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::memory::InProcessMemory;
use crate::net::NetSharedExt;
use crate::net::{self, net_clock_ms};
use crate::puppet::actor::Puppet;
use crate::puppet::game_calls::GameCalls;
use crate::puppet::pack_guard::{ensure_pack_guard, guard, has_free_slot, still_guarded, unguard};

const POWER_UNIQUE_ID_OFFSET: u32 = 0xA000;

static POWERS: LazyLock<Mutex<PartnerPowers>> = LazyLock::new(Mutex::default);

#[derive(Debug)]
struct ShownPower {
    name: CString,
    actor: Puppet,
    last_started: Option<i32>,
}

#[derive(Debug, Default)]
struct PartnerPowers {
    table: PowerModelTable,
    latest: BTreeMap<u16, PowerModelState>,
    motions: BTreeMap<u16, MotionState>,
    casts: PowerCastLog,
    shown: BTreeMap<u16, ShownPower>,
    gates: BTreeMap<u16, RespawnGate>,
    requested: Vec<CString>,
    last_note: Option<String>,
}

impl PartnerPowers {
    fn clear(&mut self, game: &Replicant<InProcessMemory>) {
        for instance in self.table.take_all() {
            self.remove(game, instance, "partner not in play");
        }
        let leftovers: Vec<u16> = self.shown.keys().copied().collect();
        for instance in leftovers {
            self.remove(game, instance, "partner not in play");
        }
    }

    fn remove(&mut self, game: &Replicant<InProcessMemory>, instance: u16, why: &str) {
        self.latest.remove(&instance);
        self.motions.remove(&instance);
        self.gates.remove(&instance);
        if let Some(cast) = self.casts.end(instance, net_clock_ms()) {
            log_line(&cast.describe("partner"));
        }
        let Some(shown) = self.shown.remove(&instance) else {
            return;
        };
        if still_guarded(shown.actor.actor()) {
            unguard(shown.actor.actor());
            let live = !shown.actor.liveness_is_trusted() || shown.actor.is_live(game);
            if live {
                shown.actor.despawn_detached(game);
            }
        }
        log_line(&format!(
            "puppet power: {:?} #{instance} removed ({why})",
            shown.name
        ));
    }

    fn forget_vanished(&mut self, game: &Replicant<InProcessMemory>) {
        let gone: Vec<u16> = self
            .shown
            .iter()
            .filter(|(_, shown)| {
                !still_guarded(shown.actor.actor())
                    || (shown.actor.liveness_is_trusted() && !shown.actor.is_live(game))
            })
            .map(|(instance, _)| *instance)
            .collect();
        for instance in gone {
            if let Some(shown) = self.shown.remove(&instance) {
                unguard(shown.actor.actor());
                self.requested.retain(|requested| *requested != shown.name);
                log_line(&format!(
                    "puppet power: {:?} #{instance} is gone, will show it again when its pack is back",
                    shown.name
                ));
            }
        }
    }

    fn try_show(
        &mut self,
        build: &VerifiedBuild,
        game: &Replicant<InProcessMemory>,
        state: PowerModelState,
        now_ms: u64,
    ) {
        let Ok(name) = CString::new(power_model_pack_name(state.kind)) else {
            return;
        };
        if !ensure_pack_guard(build) {
            self.note("puppet power: release guard missing, power models stay hidden");
            return;
        }
        let calls = GameCalls::new(build);
        let pack = calls.find_model_pack(&name);
        if pack != 0 {
            self.requested.retain(|requested| *requested != name);
        }
        if pack == 0 {
            if !self.requested.contains(&name) {
                let slot = calls.request_model_pack(&name);
                log_line(&format!("puppet power: loading {name:?} (slot {slot:#x})"));
                self.requested.push(name);
            }
            return;
        }
        let gate = self.gates.entry(state.instance).or_default();
        if !gate.may_spawn(true, now_ms) {
            return;
        }
        if !has_free_slot() {
            self.note("puppet power: no free pack guard slot, power model stays hidden");
            return;
        }
        let unique_id = PUPPET_UNIQUE_ID_BASE
            .saturating_add_unsigned(POWER_UNIQUE_ID_OFFSET)
            .saturating_add_unsigned(u32::from(state.instance));
        let matrix = pose_of(state).creation_matrix();
        let spawned = if power_model_is_prop(state.kind) {
            Puppet::spawn_model_by_class_id_only(build, &name, unique_id, &matrix)
        } else {
            Puppet::spawn_model(build, &name, unique_id, &matrix)
        };
        let actor = match spawned {
            Ok(actor) => actor,
            Err(error) => {
                self.note(&format!("puppet power: cannot spawn {name:?} ({error})"));
                return;
            }
        };
        let worn = game
            .memory()
            .read_u64(actor.actor() + ACTOR_MODEL_PACK_OFFSET)
            .unwrap_or(0);
        guard(actor.actor(), if worn == 0 { pack } else { worn });
        log_line(&format!(
            "puppet power: showing {name:?} #{} as actor {:#x} at {:?}, pack {pack:#x}, worn {worn:#x}",
            state.instance,
            actor.actor(),
            state.position
        ));
        self.casts.begin(
            state.instance,
            power_model_pack_name(state.kind),
            net_clock_ms(),
        );
        self.shown.insert(
            state.instance,
            ShownPower {
                name,
                actor,
                last_started: None,
            },
        );
    }

    fn place(
        &mut self,
        build: &VerifiedBuild,
        game: &Replicant<InProcessMemory>,
        state: PowerModelState,
    ) {
        let motion = self.motions.get(&state.instance).copied();
        let Some(shown) = self.shown.get_mut(&state.instance) else {
            return;
        };
        if let Err(error) = shown
            .actor
            .set_matrix(game, &pose_of(state).creation_matrix())
        {
            log_line(&format!("puppet power: cannot place ({error})"));
        }
        let Some(motion) = motion else {
            return;
        };
        if follow_motion(build, shown, motion) {
            self.casts.motion(state.instance, motion.index);
        }
    }

    fn note(&mut self, message: &str) {
        if self.last_note.as_deref() != Some(message) {
            log_line(message);
            self.last_note = Some(message.to_owned());
        }
    }
}

pub fn tick_partner_powers(build: &VerifiedBuild) {
    let Some(net) = net::handle() else {
        return;
    };
    let arrived = net.take_relayed(|message| {
        matches!(
            message,
            Message::PowerModel(_) | Message::PowerMotion(_) | Message::PowerEnded(_)
        )
    });
    let game = Replicant::new(InProcessMemory::new(build));
    let mut powers = POWERS.lock().unwrap_or_else(PoisonError::into_inner);
    if !net.view().partner_in_play() {
        powers.clear(&game);
        return;
    }
    let now_ms = net_clock_ms();
    for message in arrived {
        match message {
            Message::PowerModel(state) => {
                if powers.table.note(state.instance, now_ms) {
                    powers.latest.insert(state.instance, state);
                } else {
                    powers.note("puppet power: too many partner models at once, one is skipped");
                }
            }
            Message::PowerMotion(motion) => {
                powers.motions.insert(motion.instance, motion.motion);
            }
            Message::PowerEnded(ended) => {
                powers.table.forget(ended.instance);
                let why = format!("partner's cast ended after {} ms", ended.lasted_ms);
                powers.remove(&game, ended.instance, &why);
            }
            _ => {}
        }
    }
    for instance in powers.table.take_stale(now_ms) {
        powers.remove(&game, instance, "no longer sent");
    }
    powers.forget_vanished(&game);
    let wanted: Vec<PowerModelState> = powers.latest.values().copied().collect();
    for state in wanted {
        if !powers.shown.contains_key(&state.instance) {
            powers.try_show(build, &game, state, now_ms);
        }
        powers.place(build, &game, state);
    }
}

pub fn power_actor_addresses() -> Vec<u64> {
    let Ok(powers) = POWERS.try_lock() else {
        return Vec::new();
    };
    powers
        .shown
        .values()
        .map(|shown| shown.actor.actor())
        .collect()
}

fn follow_motion(build: &VerifiedBuild, shown: &mut ShownPower, target: MotionState) -> bool {
    let index = i32::from(target.index);
    let mut memory = InProcessMemory::new(build);
    let layer = read_npc_body_layer(&memory, shown.actor.actor())
        .ok()
        .flatten();
    match follow_step(layer.map(|layer| layer.motion), shown.last_started, index) {
        MotionStep::Start { blend } => {
            shown.actor.play_motion(
                BODY_MOTION_LAYER,
                index,
                blend,
                target.looping,
                target.speed,
                target.frame,
            );
            if shown.last_started != Some(index) {
                log_line(&format!(
                    "puppet power: {:?} plays motion {index} from frame {:.1} (looping {}, was {})",
                    shown.name,
                    target.frame,
                    target.looping,
                    layer.map_or(-1, |layer| layer.motion.index)
                ));
            }
            shown.last_started = Some(index);
            true
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
            false
        }
    }
}

fn pose_of(state: PowerModelState) -> PuppetPose {
    PuppetPose {
        position: replicant_game::Position {
            x: state.position[0],
            y: state.position[1],
            z: state.position[2],
        },
        yaw: state.yaw_radians,
    }
}
