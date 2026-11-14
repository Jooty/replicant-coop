use std::sync::{Mutex, PoisonError};

use replicant_game::fight_sync::{SlotState, script_slot_state};
use replicant_game::script_functions::shows_modal_ui;
use replicant_game::shared_events::phase_name;
use replicant_game::story_relay::{
    CallKey, LongRelayWait, MAX_WAITING_RELAY_CALLS, QueueRetry, RelayCallFlight, RelayCallGate,
    RelayCallNotice, RelayCallSubmitted, WaitingRelayCall,
};
use replicant_proto::StoryCall;

use crate::build_gate::VerifiedBuild;
use crate::cutscene_skip::{close_menus_for_modal_entry, expect_handler_run, forget_handler_run};
use crate::diagnostics::log_line;
use crate::game_hooks::{last_native, last_pop};
use crate::net::net_clock_ms;
use crate::stay_together::{main_menu_open, pause_menu_open};
use crate::story_relay::inject::{queue_call_now, queue_call_unmarked};
use crate::story_relay::state::LEDGER;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RelayPath {
    CallbackFollow,
    MessageHandler,
    SharedFunction,
    SceneFollow,
    StoryEvent(CallKey),
    HeldOwnCall,
}

static RELAY_CALLS: Mutex<RelayCallGate<RelayPath>> = Mutex::new(RelayCallGate::new());

pub(crate) fn submit_relay_call(call: &StoryCall, path: RelayPath) -> bool {
    let submitted = RELAY_CALLS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .submit(call.clone(), path, net_clock_ms());
    match submitted {
        RelayCallSubmitted::Waiting { position } => {
            if position > 1 {
                log_line(&format!(
                    "relay calls: {} waits at position {position}, one partner call goes into your game's queue at a time",
                    call.function
                ));
            }
            true
        }
        RelayCallSubmitted::Full => {
            log_line(&format!(
                "relay calls: {} not queued, {MAX_WAITING_RELAY_CALLS} partner calls wait already",
                call.function
            ));
            false
        }
    }
}

pub(crate) fn relay_queue_has_room() -> bool {
    RELAY_CALLS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .waiting_len()
        < MAX_WAITING_RELAY_CALLS
}

pub fn tick_relay_calls(build: &VerifiedBuild) {
    let manager_state = crate::fight_sync::script_manager_state(build);
    let slot = script_slot_state(manager_state);
    let now = net_clock_ms();
    let fight_call_queued = crate::fight_sync::own_call_queued();
    let menu_open = main_menu_open(build) || pause_menu_open(build);
    let (due, long_wait, in_flight) = {
        let mut gate = RELAY_CALLS.lock().unwrap_or_else(PoisonError::into_inner);
        gate.check_queued(slot, now, |name, arguments| {
            crate::fight_sync::queued_call_is_ours(build, name, arguments)
        });
        let due = gate.tick(slot, fight_call_queued, |call| {
            shows_modal_ui(phase_name(call.phase).unwrap_or_default(), &call.function)
        });
        let long_wait = gate.long_wait(menu_open, now);
        // Only a long wait's log reads the call in flight, so it is copied only then.
        let in_flight = long_wait.as_ref().map(|_| gate.in_flight().clone());
        (due, long_wait, in_flight)
    };
    if let (Some(wait), Some(in_flight)) = (long_wait, in_flight) {
        log_long_wait(&wait, &in_flight, fight_call_queued, slot, manager_state);
    }
    if let Some(due) = due {
        queue_now(build, due, now);
    }
    let notices = RELAY_CALLS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take_notices();
    for notice in notices {
        log_line(&match notice {
            RelayCallNotice::QueuedCallNeverPopped { name, waited_ms } => format!(
                "relay calls: the partner's {name} was queued here but did not pop within {waited_ms} ms and left the queue, the next call may go"
            ),
            RelayCallNotice::QueuedCallStillWaiting { name, waited_ms } => format!(
                "relay calls: the partner's {name} is still in your game's queue after {waited_ms} ms, the next call waits for it"
            ),
            RelayCallNotice::QueueingGaveUp { name, tries } => format!(
                "relay calls: your game's queue refused the partner's {name} {tries} times, dropped"
            ),
        });
    }
}

fn log_long_wait(
    wait: &LongRelayWait,
    in_flight: &RelayCallFlight,
    other_call_queued: bool,
    slot: SlotState,
    manager_state: Option<i32>,
) {
    let in_flight = match in_flight {
        RelayCallFlight::Idle => "nothing".to_owned(),
        RelayCallFlight::Queued {
            name, waited_ms, ..
        } => format!("{name} queued {waited_ms} ms"),
        RelayCallFlight::Running { name } => format!("{name} running"),
    };
    let state = manager_state.map_or_else(|| "unread".to_owned(), |state| format!("{state:#x}"));
    let last_pop = last_pop().map_or_else(
        || "no pop yet".to_owned(),
        |(name, at)| format!("{name} {} ms ago", net_clock_ms().saturating_sub(at)),
    );
    let last_native = last_native().map_or_else(
        || "none hooked since that pop".to_owned(),
        |(native, ago)| format!("{native} {ago} ms ago"),
    );
    log_line(&format!(
        "relay calls: the partner's {} waited {} ms first in line ({} waiting), in flight {in_flight}, fight call queued {other_call_queued}, slot {slot:?} (state {state}), last pop {last_pop}, its last native {last_native}",
        wait.front, wait.waited_ms, wait.waiting
    ));
}

fn queue_now(build: &VerifiedBuild, due: WaitingRelayCall<RelayPath>, now: u64) {
    let call = &due.call;
    let name = call.function.as_str();
    let first_try = RELAY_CALLS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .queue_tries()
        == 0;
    if first_try {
        close_menus_for_modal_entry(build, call.phase, name);
    }
    if due.then == RelayPath::MessageHandler {
        expect_handler_run(name, now);
    }
    let queued = if due.then == RelayPath::HeldOwnCall {
        queue_call_unmarked(build, call)
    } else {
        queue_call_now(build, call, Some(due.submitted_ms))
    };
    let Some(slot) = queued else {
        if due.then == RelayPath::MessageHandler {
            forget_handler_run(name, now);
        }
        let path = due.then;
        let name = name.to_owned();
        let retry = RELAY_CALLS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .queue_failed(due);
        if retry == (QueueRetry::Again { tries: 1 }) {
            log_line(&format!(
                "relay calls: could not queue the partner's {name} ({path:?}), your game's queue is full, tried again each tick"
            ));
        }
        return;
    };
    RELAY_CALLS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .queued(name, slot);
    if let RelayPath::StoryEvent(key) = due.then {
        LEDGER
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .note_injected(slot, key, now);
    }
    log_line(&format!(
        "relay calls: the partner's {name} queued here ({:?}, waited {} ms)",
        due.then,
        now.saturating_sub(due.submitted_ms)
    ));
}

/// Notes a pop; true when it is the call this gate queued (a partner's call or a released own
/// one), not one the game fired by itself.
pub(crate) fn relay_call_popped(arguments: u64) -> bool {
    RELAY_CALLS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .popped(arguments)
}

pub(super) fn relay_call_cancelled(arguments: u64) {
    RELAY_CALLS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .cancelled(arguments);
}

pub(crate) fn relay_calls_are_clear() -> bool {
    RELAY_CALLS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_clear()
}

pub(crate) fn relay_calls_enter_phase(phase: Option<u16>) {
    let dropped = RELAY_CALLS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .enter_phase(phase);
    if dropped > 0 {
        log_line(&format!(
            "relay calls: {dropped} waiting partner call(s) dropped, you are in another phase now"
        ));
    }
}

pub(crate) fn forget_relay_call_in_flight(why: &str) {
    let forgotten = RELAY_CALLS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .forget_in_flight();
    if let Some(name) = forgotten {
        log_line(&format!(
            "relay calls: the partner's {name} is no longer tracked in your game's queue ({why})"
        ));
    }
}

pub(crate) fn drop_relay_calls(why: &str) {
    let dropped = RELAY_CALLS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    if dropped > 0 {
        log_line(&format!(
            "relay calls: {dropped} waiting partner call(s) dropped ({why})"
        ));
    }
}
