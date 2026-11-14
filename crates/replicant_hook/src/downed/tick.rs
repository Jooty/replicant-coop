use std::sync::atomic::Ordering;
use std::sync::{Mutex, PoisonError};
use std::time::Instant;

use replicant_game::Replicant;
use replicant_game::downed::{
    DownedRules, DownedState, KillGuardInputs, PlayerLock, StepInput, kill_is_blocked,
};
use replicant_game::stay_together::FollowRole;
use replicant_proto::Message;

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::downed::apply::{
    OwnedBits, Pending, apply_effects, heal_after_load, hold_endure, release_bits, resolve_backup,
    resolve_pending, resolve_rewind,
};
use crate::downed::player_lock::apply_player_lock;
use crate::downed::status::{DownedStatus, GO_DOWN_REQUESTED, REVIVE_REQUESTED, publish};
use crate::downed::{MAX_STEP_SECONDS, PARTNER_STATE_MAX_AGE};
use crate::event_log::post_event;
use crate::game_hooks::take_lethal_hit_seen;
use crate::memory::InProcessMemory;
use crate::net::NetSharedExt;
use crate::net::{self, NetShared};

static RUNTIME: Mutex<Runtime> = Mutex::new(Runtime {
    rules: DownedRules::new(),
    last_tick: None,
    owned: None,
    lock: PlayerLock::new(),
    pending: Pending {
        leave_downed: false,
        kill: false,
        backup_waited: None,
        rewind_waited: None,
        heal_watch: None,
    },
    announced: (DownedState::Up, DownedState::Up),
});

struct Runtime {
    rules: DownedRules,
    last_tick: Option<Instant>,
    owned: Option<OwnedBits>,
    lock: PlayerLock,
    pending: Pending,
    announced: (DownedState, DownedState),
}

pub(super) fn kill_is_blocked_now() -> bool {
    let Some(net) = net::handle() else {
        return false;
    };
    let session_active = net.view().partner_in_play();
    let Ok(runtime) = RUNTIME.try_lock() else {
        return false;
    };
    kill_is_blocked(KillGuardInputs {
        session_active,
        partner_downed: runtime.rules.partner() == DownedState::Downed,
        game_over: runtime.rules.is_game_over(),
        rewinding: runtime.rules.is_rewinding(),
    })
}

pub fn tick_downed(build: &VerifiedBuild) {
    let Some(net) = net::handle() else {
        let mut runtime = RUNTIME.lock().unwrap_or_else(PoisonError::into_inner);
        apply_player_lock(build, &mut runtime.lock, false);
        return;
    };
    let view = net.view();
    let session_active = view.partner_in_play();
    let lethal_hit_seen = take_lethal_hit_seen();
    let mut runtime = RUNTIME.lock().unwrap_or_else(PoisonError::into_inner);
    let now = Instant::now();
    let dt_seconds = runtime.last_tick.replace(now).map_or(0.0, |last| {
        now.saturating_duration_since(last)
            .as_secs_f32()
            .min(MAX_STEP_SECONDS)
    });
    let partner_events = net.take_relayed(|message| matches!(message, Message::Combat(_)));
    let mut game = Replicant::new(InProcessMemory::new(build));
    let mut effects = Vec::new();
    runtime
        .rules
        .set_guest(FollowRole::from_slot(view.local_slot) == FollowRole::Guest);
    if session_active {
        for message in partner_events {
            if let Message::Combat(event) = message {
                effects.extend(runtime.rules.on_partner_event(event));
            }
        }
        if GO_DOWN_REQUESTED.swap(false, Ordering::Relaxed) {
            effects.extend(runtime.rules.request_go_down());
        }
        if REVIVE_REQUESTED.swap(false, Ordering::Relaxed) {
            effects.extend(runtime.rules.request_revive_partner());
        }
    }
    if let Ok(local_health) = game.health() {
        effects.extend(runtime.rules.step(StepInput {
            dt_seconds,
            session_active,
            local_health,
            lethal_hit_seen,
            partner_distance: partner_distance(&view),
        }));
    }
    let runtime = &mut *runtime;
    apply_effects(
        net,
        &mut game,
        &mut runtime.owned,
        &mut runtime.pending,
        &effects,
    );
    resolve_pending(build, &mut game, &mut runtime.owned, &mut runtime.pending);
    let partner_name = view.partner_name().unwrap_or("The other player");
    resolve_rewind_and_backup(build, runtime, &mut game, dt_seconds, net, partner_name);
    maintain_endure(runtime, &mut game, session_active);
    let wanted_lock = session_active && runtime.rules.locks_controls();
    apply_player_lock(build, &mut runtime.lock, wanted_lock);
    announce_changes(
        &mut runtime.announced,
        (runtime.rules.local(), runtime.rules.partner()),
        view.partner_name().unwrap_or("The other player"),
    );
    publish(DownedStatus {
        session_active,
        local: runtime.rules.local(),
        partner: runtime.rules.partner(),
        revive_progress: runtime.rules.revive_progress(),
        bleed_out_progress: runtime.rules.bleed_out_progress(),
        incoming_revive_progress: runtime.rules.incoming_revive_progress(),
        game_over: runtime.rules.is_game_over(),
        rewinds: crate::rewind::rewind_count(),
    });
}

fn resolve_rewind_and_backup(
    build: &VerifiedBuild,
    runtime: &mut Runtime,
    game: &mut Replicant<InProcessMemory>,
    dt_seconds: f32,
    net: &crate::net::NetThread,
    host_name: &str,
) {
    resolve_backup(
        build,
        game,
        &mut runtime.rules,
        &mut runtime.pending,
        dt_seconds,
    );
    let step = resolve_rewind(
        build,
        game,
        &mut runtime.owned,
        &mut runtime.rules,
        &mut runtime.pending,
        dt_seconds,
    );
    if step.rewound {
        post_event(&format!("Going back to {host_name}'s checkpoint"));
    }
    apply_effects(
        net,
        game,
        &mut runtime.owned,
        &mut runtime.pending,
        &step.effects,
    );
    heal_after_load(game, &mut runtime.pending, dt_seconds);
}

fn announce_changes(
    announced: &mut (DownedState, DownedState),
    current: (DownedState, DownedState),
    partner_name: &str,
) {
    let (was_local, was_partner) = *announced;
    *announced = current;
    match (was_local, current.0) {
        (DownedState::Up, DownedState::Downed) => post_event("You were downed"),
        (DownedState::Downed, DownedState::Up) => {
            post_event(&format!("{partner_name} revived you"))
        }
        _ => {}
    }
    match (was_partner, current.1) {
        (DownedState::Up, DownedState::Downed) => post_event(&format!("{partner_name} was downed")),
        (DownedState::Downed, DownedState::Up) => {
            post_event(&format!("You revived {partner_name}"))
        }
        _ => {}
    }
}

fn maintain_endure(
    runtime: &mut Runtime,
    game: &mut Replicant<InProcessMemory>,
    session_active: bool,
) {
    let holding = session_active
        && !runtime.rules.is_game_over()
        && !runtime.pending.kill
        && !runtime.pending.leave_downed;
    let result = if holding {
        hold_endure(game, &mut runtime.owned)
    } else if runtime.pending.leave_downed || runtime.pending.kill {
        Ok(())
    } else {
        release_bits(game, &mut runtime.owned)
    };
    if let Err(error) = result {
        log_line(&format!("downed: life flags not writable ({error})"));
    }
}

fn partner_distance(view: &NetShared) -> Option<f32> {
    let local = view.local.as_ref()?;
    let remote = view.partner_state()?;
    if remote.age() > PARTNER_STATE_MAX_AGE || remote.state.phase != local.phase {
        return None;
    }
    let squared: f32 = local
        .position
        .iter()
        .zip(remote.state.position)
        .map(|(a, b)| (a - b) * (a - b))
        .sum();
    squared.is_finite().then(|| squared.sqrt())
}
