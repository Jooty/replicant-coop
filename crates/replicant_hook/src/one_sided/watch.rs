use std::panic;
use std::sync::{Mutex, OnceLock, PoisonError};

use replicant_game::PHASE_TABLE;
use replicant_game::one_sided::{
    OneSidedTracker, Side, Verdict, is_watched_callback, one_sided_log_line,
};
use replicant_game::stay_together::FollowRole;
use replicant_game::story_relay::pack_hash_of_phase;
use replicant_game::{PhaseEntry, Replicant};
use replicant_proto::{CallbackFired, Message};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::memory::InProcessMemory;
use crate::net::NetSharedExt;
use crate::net::{self, NetState, net_clock_ms};
use crate::story_relay::shared_story_events;

static TRACKER: Mutex<OneSidedTracker> = Mutex::new(OneSidedTracker::new());
static HOOKED_BUILD: OnceLock<VerifiedBuild> = OnceLock::new();

pub fn note_fired(name: &str) {
    let Some(build) = HOOKED_BUILD.get() else {
        return;
    };
    let _ = panic::catch_unwind(|| watch_local_fire(build, name));
}

pub fn tick(build: &VerifiedBuild) {
    HOOKED_BUILD.get_or_init(|| *build);
    let Some(net) = net::handle() else {
        return;
    };
    let arrived = net.take_relayed(|message| matches!(message, Message::WatchedCallbackFired(_)));
    let view = net.view();
    if view.state != NetState::Connected || !shared_story_events() {
        TRACKER
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        return;
    }
    let role = FollowRole::from_slot(view.local_slot);
    let now_ms = net_clock_ms();
    let mut verdicts = Vec::new();
    {
        let mut tracker = TRACKER.lock().unwrap_or_else(PoisonError::into_inner);
        for message in arrived {
            if let Message::WatchedCallbackFired(fired) = message {
                verdicts.extend(tracker.note(Side::Partner, &fired.name, fired.phase, now_ms));
            }
        }
        verdicts.extend(tracker.expire(now_ms));
    }
    for verdict in verdicts {
        log_verdict(&verdict, role);
    }
}

fn watch_local_fire(build: &VerifiedBuild, name: &str) {
    let Some(net) = net::handle() else {
        return;
    };
    let view = net.view();
    if !shared_story_events() || !view.partner_in_play() {
        return;
    }
    let game = Replicant::new(InProcessMemory::new(build));
    let Ok(phase_name) = game.current_phase() else {
        return;
    };
    if !is_watched_callback(pack_hash_of_phase(&phase_name), name) {
        return;
    }
    let Some(phase) = PhaseEntry::index_of(&phase_name).and_then(|index| u16::try_from(index).ok())
    else {
        return;
    };
    let partner_here = view
        .partner_state()
        .is_some_and(|remote| remote.state.phase == phase);
    if !partner_here {
        return;
    }
    net.send_relayed(Message::WatchedCallbackFired(CallbackFired {
        phase,
        name: name.to_owned(),
    }));
    let verdict = TRACKER.lock().unwrap_or_else(PoisonError::into_inner).note(
        Side::Here,
        name,
        phase,
        net_clock_ms(),
    );
    if let Some(verdict) = verdict {
        log_verdict(&verdict, FollowRole::from_slot(view.local_slot));
    }
}

fn log_verdict(verdict: &Verdict, role: FollowRole) {
    let zone = match verdict {
        Verdict::BothRan { .. } => "",
        Verdict::OneSided { phase, .. } => PHASE_TABLE
            .get(usize::from(*phase))
            .map_or("unknown zone", |entry| entry.name),
    };
    log_line(&one_sided_log_line(verdict, role, zone));
}
