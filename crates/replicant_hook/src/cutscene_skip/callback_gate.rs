use std::panic;
use std::sync::{Mutex, PoisonError};

use replicant_game::GameMemory;
use replicant_game::callback_gate::{
    CALLBACK_GATE_HOST_GRACE_MS, CallbackGate, EndArrival, EndFollow, EndQueue, GateRole, GateStep,
    LocalFire, MaskIdleTimer, ParkedCall, PartnerFire, PartnerStartedCopies, PlayedStart,
    QueueEvent, RanEnds, RelayedScenesPlayed, Released, TALK_START_AFTER_COMMAND_MS,
    control_slots_free, end_follow, event_base, is_end_callback, is_event_talk, is_gated_callback,
};
use replicant_game::marker::{CONTROL_MASK_FIRST_RVA, read_world_flags};
use replicant_game::message_handlers::HandlerFire;
use replicant_game::script_functions::{ClearCause, LateCopy, PhaseLoadWatch};
use replicant_game::shared_events::{
    SharedEventsRan, SharedFire, StartPath, is_shared_event, phase_name, start_path,
};
use replicant_game::stay_together::FollowRole;
use replicant_game::story_calls::is_event_callback_name;
use replicant_proto::{CallbackFired, Message, StoryCall};

use crate::build_gate::VerifiedBuild;
use crate::cutscene_skip::message_handlers::{
    judge_popped, receive_message_handler, reset as reset_message_handlers,
};
use crate::cutscene_skip::restart_signals::restart_signals;
use crate::cutscene_skip::scene_follow::{local_phase, note_injected};
use crate::cutscene_skip::shared_functions::{
    close_menus_for_modal_entry, late_copy_verdict_here, receive_shared_function,
    reset_shared_functions, run_due_fill_ins, set_local_phase,
};
use crate::diagnostics::log_line;
use crate::memory::InProcessMemory;
use crate::net::NetSharedExt;
use crate::net::{self, NetShared, NetState, NetThread, net_clock_ms};
use crate::stay_together::{close_main_menu, close_pause_menu, main_menu_open, pause_menu_open};
use crate::story_relay::{
    RelayPath, drop_relay_calls, forget_relay_call_in_flight,
    own_scene_start_waits_for_the_partner, relay_calls_are_clear, relay_calls_enter_phase,
    run_parked, submit_relay_call,
};
use crate::story_writes::begin_end_watch;

static LOAD_WATCH: Mutex<PhaseLoadWatch> = Mutex::new(PhaseLoadWatch::new());
static GATE: Mutex<CallbackGate> = Mutex::new(CallbackGate::new());
pub(super) static WAITING_ENDS: Mutex<EndQueue> = Mutex::new(EndQueue::new());
static STARTED_FROM_PARTNER: Mutex<PartnerStartedCopies> = Mutex::new(PartnerStartedCopies::new());
static MASK_IDLE: Mutex<MaskIdleTimer> = Mutex::new(MaskIdleTimer::new());
static RAN_ENDS: Mutex<RanEnds> = Mutex::new(RanEnds::new());
static SHARED_RAN: Mutex<SharedEventsRan> = Mutex::new(SharedEventsRan::new());
static RELAYED_SCENES: Mutex<RelayedScenesPlayed> = Mutex::new(RelayedScenesPlayed::new());
static LATEST_PARTNER_END: Mutex<Option<String>> = Mutex::new(None);
static LAST_TALK_COMMAND_MS: Mutex<Option<u64>> = Mutex::new(None);

static LAST_PLAYER_TALK_COMMAND_MS: Mutex<Option<u64>> = Mutex::new(None);

pub fn take_player_talk_command() -> bool {
    let noted = LAST_PLAYER_TALK_COMMAND_MS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    noted.is_some_and(|at| net_clock_ms().saturating_sub(at) <= TALK_START_AFTER_COMMAND_MS)
}

pub fn note_talk_command() {
    *LAST_PLAYER_TALK_COMMAND_MS
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(net_clock_ms());
    *LAST_TALK_COMMAND_MS
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(net_clock_ms());
}

fn recent_talk_command() -> bool {
    let noted = *LAST_TALK_COMMAND_MS
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    noted.is_some_and(|at| net_clock_ms().saturating_sub(at) <= TALK_START_AFTER_COMMAND_MS)
}

fn take_recent_talk_command() -> bool {
    let noted = LAST_TALK_COMMAND_MS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    noted.is_some_and(|at| net_clock_ms().saturating_sub(at) <= TALK_START_AFTER_COMMAND_MS)
}

pub fn latest_partner_end() -> Option<(String, bool)> {
    let name = LATEST_PARTNER_END
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()?;
    let ran_here = end_ran_here(&name);
    Some((name, ran_here))
}

pub fn end_ran_here(name: &str) -> bool {
    RAN_ENDS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .contains_name(name)
}

pub(crate) fn note_relayed_scene_played(phase: u16, function: &str) {
    let noted = RELAYED_SCENES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .note_played(phase, function);
    if noted {
        log_line(&format!(
            "callback follow: {function} played here from the story relay, a later start of its event in this phase does not run"
        ));
    }
}

pub(crate) fn drop_start_of_a_played_scene(name: &str) -> bool {
    if !is_gated_callback(name) {
        return false;
    }
    let Some(net) = net::handle() else {
        return false;
    };
    let Some(phase) = net.view().local.as_ref().map(|local| local.phase) else {
        return false;
    };
    let verdict = RELAYED_SCENES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .judge_start(phase, name);
    match verdict {
        PlayedStart::Run => false,
        PlayedStart::Drop { first_for_start } => {
            if first_for_start {
                log_line(&format!(
                    "callback follow: your game fired {name}, but its scene already played here from the story relay in this phase, dropped (no second scene, no second spawn, no fight window from it)"
                ));
            }
            true
        }
    }
}

fn relayed_scene_played(phase: u16, start: &str) -> bool {
    RELAYED_SCENES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .played(phase, start)
}

pub(super) fn allow_next(name: &str) {
    GATE.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .allow_next(name, net_clock_ms());
}

pub(crate) fn forget_gate_copies(name: &str) {
    GATE.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .ran_from_the_fight_stream(name);
}

pub fn hold_fired_callback(name: &str) -> bool {
    crate::cutscene_skip::shared_functions::note_popped_callback(name);
    match judge_popped(name) {
        HandlerFire::HostRunsAndSends | HandlerFire::GuestRunsTheHostsCall => return false,
        HandlerFire::GuestHoldsForTheHost => return true,
        HandlerFire::NotAHandler | HandlerFire::RunHereOnly => {}
    }
    if is_end_callback(name) {
        let _ = panic::catch_unwind(|| announce_end(name));
        return false;
    }
    if !is_gated_callback(name) {
        if is_event_talk(name) && !recent_talk_command() {
            let _ = panic::catch_unwind(|| note_talk_running_from_partner(name));
        }
        return false;
    }
    let after_talk_command = is_event_callback_name(name) && take_recent_talk_command();
    match start_path(current_phase_name().as_deref(), name, after_talk_command) {
        StartPath::YourTalk => decide_talk(name, true),
        StartPath::SceneTalk => decide_talk(name, false),
        StartPath::SharedEvent => panic::catch_unwind(|| decide_shared(name)).unwrap_or(false),
        StartPath::Cutscene => panic::catch_unwind(|| decide(name, false)).unwrap_or(false),
    }
}

fn note_talk_running_from_partner(name: &str) {
    let Some(net) = net::handle() else {
        return;
    };
    let view = net.view();
    if gate_role(&view) != Some(GateRole::Guest) {
        return;
    }
    let Some(phase) = shared_phase(&view) else {
        return;
    };
    STARTED_FROM_PARTNER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .note(name, phase);
    log_line(&format!(
        "callback follow: {name} started here without a talk of yours, the partner's End of it will close this copy"
    ));
}

fn current_phase_name() -> Option<String> {
    let net = net::handle()?;
    let view = net.view();
    let phase = view.local.as_ref()?.phase;
    phase_name(phase).map(str::to_owned)
}

fn decide_shared(name: &str) -> bool {
    let Some(net) = net::handle() else {
        return false;
    };
    let view = net.view();
    let (Some(_), Some(phase)) = (gate_role(&view), shared_phase(&view)) else {
        return false;
    };
    let base = event_base(name);
    let led_by_partner = GATE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .partner_led(name, net_clock_ms());
    let ruling = SHARED_RAN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .judge(phase, base, led_by_partner);
    if let SharedFire::Drop { first_for_base } = ruling {
        if first_for_base {
            log_line(&format!(
                "shared event: {name} already ran on the partner's game, dropped here"
            ));
        }
        return true;
    }
    let held = decide(name, true);
    if !held {
        SHARED_RAN
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .note_ran(phase, base);
    }
    held
}

fn note_partner_ran_shared_event(phase: u16, name: &str) {
    let Some(phase_label) = phase_name(phase) else {
        return;
    };
    if is_shared_event(phase_label, name) {
        SHARED_RAN
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .note_ran(phase, event_base(name));
    }
}

fn decide_talk(name: &str, after_talk_command: bool) -> bool {
    if after_talk_command {
        log_line(&format!("talk owner: {name} is yours, runs here"));
    } else {
        log_line(&format!(
            "talk owner: {name} is a scene talk, runs here only"
        ));
    }
    false
}

fn announce_end(name: &str) {
    let Some(net) = net::handle() else {
        return;
    };
    let view = net.view();
    let Some(role) = gate_role(&view) else {
        return;
    };
    let Some(phase) = shared_phase(&view) else {
        return;
    };
    note_ran_end(name, phase);
    let started_from_partner = STARTED_FROM_PARTNER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take(name, phase);
    if started_from_partner {
        return;
    }
    let receiver = if role == GateRole::Guest {
        "host"
    } else {
        "guest"
    };
    log_line(&format!(
        "callback follow: your game ended {name}, sent to the {receiver}"
    ));
    net.send_relayed(Message::CallbackFired(CallbackFired {
        phase,
        name: name.to_owned(),
    }));
    if role == GateRole::Guest {
        begin_end_watch();
    }
}

pub(super) fn note_ran_end(name: &str, phase: u16) {
    RAN_ENDS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .note(name, phase);
}

fn partner_label(role: GateRole) -> &'static str {
    match role {
        GateRole::Host => "guest",
        GateRole::Guest => "host",
    }
}

fn partner_end_verdict(
    build: &VerifiedBuild,
    view: &NetShared,
    name: &str,
    phase: u16,
) -> EndFollow {
    let arrival = EndArrival {
        message_phase: phase,
        shared_phase: shared_phase(view),
        local_phase: local_phase(build),
        local_in_own_talk: read_world_flags(&InProcessMemory::new(build))
            .is_ok_and(|flags| flags.talk_window_open),
        control_mask_settled: control_mask_settled(build),
        already_ran_here: RAN_ENDS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(name, phase),
        own_copy_of_same_event_running: STARTED_FROM_PARTNER
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(name, phase),
    };
    end_follow(&arrival)
}

pub(crate) fn control_slots_free_now(build: &VerifiedBuild) -> bool {
    let memory = InProcessMemory::new(build);
    let base = memory.module_base();
    let slots: [u32; 4] = core::array::from_fn(|slot| {
        memory
            .read_i32(base + u64::from(CONTROL_MASK_FIRST_RVA) + 4 * slot as u64)
            .map_or(u32::MAX, i32::cast_unsigned)
    });
    control_slots_free(slots, main_menu_open(build) || pause_menu_open(build))
}

pub(crate) fn partner_end_waiting_for(phase: u16) -> bool {
    WAITING_ENDS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .front_end()
        .is_some_and(|end| end.phase == phase)
}

fn control_mask_settled(build: &VerifiedBuild) -> bool {
    let free = control_slots_free_now(build);
    MASK_IDLE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .observe(free, net_clock_ms())
}

fn follow_end(build: &VerifiedBuild, role: GateRole, view: &NetShared, fired: &CallbackFired) {
    let now = net_clock_ms();
    let partner = partner_label(role);
    match partner_end_verdict(build, view, &fired.name, fired.phase) {
        EndFollow::Drop(reason) if !reason.can_wait() => log_line(&format!(
            "callback follow: the {partner} ended {}, not run here: {}",
            fired.name,
            reason.reason()
        )),
        verdict => {
            let mut queue = WAITING_ENDS.lock().unwrap_or_else(PoisonError::into_inner);
            let ahead = queue.len();
            let position = queue.push(&fired.name, fired.phase, now);
            let note = match verdict {
                EndFollow::Drop(reason) => format!("waits for {}", reason.reason()),
                EndFollow::Mirror if ahead > 0 => {
                    format!("waits for the {ahead} earlier end(s) of the {partner}")
                }
                EndFollow::Mirror => "runs now".to_owned(),
            };
            log_line(&format!(
                "callback follow: the {partner} ended {}, queue position {position}, {note}",
                fired.name
            ));
        }
    }
}

fn own_copy_note(name: &str, phase: u16) -> &'static str {
    let running = STARTED_FROM_PARTNER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .contains(name, phase);
    if running {
        " (your copy of that event was started from the partner, so it ends now, no waiting)"
    } else {
        ""
    }
}

fn run_waiting_ends(build: &VerifiedBuild, role: GateRole, view: &NetShared) {
    let partner = partner_label(role);
    let now = net_clock_ms();
    let events = WAITING_ENDS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .advance(now, |end| {
            partner_end_verdict(build, view, &end.name, end.phase)
        });
    for event in events {
        match event {
            QueueEvent::Run {
                end,
                position,
                waited_ms,
            } => {
                log_line(&format!(
                    "callback follow: the {partner} ended {}, queue position {position}, runs now after waiting {}s{}",
                    end.name,
                    waited_ms / 1000,
                    own_copy_note(&end.name, end.phase)
                ));
                start(build, end.phase, &end.name, now);
            }
            QueueEvent::RunCall {
                parked,
                position,
                waited_ms,
            } => run_parked(build, &parked, position, waited_ms),
            QueueEvent::Dropped {
                end,
                position,
                reason,
            } => log_line(&format!(
                "callback follow: the {partner} ended {}, queue position {position}, not run here: {}",
                end.name,
                reason.reason()
            )),
            QueueEvent::StillWaiting {
                end,
                position,
                reason,
                waited_ms,
            } => log_line(&format!(
                "callback follow: the {partner} ended {}, queue position {position}, still waiting after {}s for {}",
                end.name,
                waited_ms / 1000,
                reason.reason()
            )),
        }
    }
}

pub(crate) fn park_partner_call(build: &VerifiedBuild, parked: ParkedCall) -> bool {
    let name = parked.call.function.clone();
    let parked_at = {
        let mut queue = WAITING_ENDS.lock().unwrap_or_else(PoisonError::into_inner);
        queue
            .park_call(parked, net_clock_ms())
            .map(|position| (position, queue.len(), queue.front_end().cloned()))
    };
    let Some((position, waiting, front)) = parked_at else {
        return false;
    };
    let behind = match (front, net::handle()) {
        (Some(end), Some(net)) => {
            let why = match partner_end_verdict(build, &net.view(), &end.name, end.phase) {
                EndFollow::Drop(reason) => reason.reason(),
                EndFollow::Mirror => "it runs on the next tick",
            };
            format!("{} ({why})", end.name)
        }
        _ => "an earlier partner call".to_owned(),
    };
    log_line(&format!(
        "partner calls: {waiting} waiting behind {behind}; {name} parked at queue position {position}"
    ));
    true
}

pub(crate) fn partner_calls_waiting() -> bool {
    !WAITING_ENDS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_empty()
        || !relay_calls_are_clear()
}

pub fn run_queued_ends_through(build: &VerifiedBuild, name: &str) -> bool {
    let taken = WAITING_ENDS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take_through(name);
    if taken.is_empty() {
        return false;
    }
    run_released(build, taken);
    true
}

pub(super) fn run_released(build: &VerifiedBuild, taken: Vec<Released>) {
    let now = net_clock_ms();
    for released in taken {
        let (end, position) = match released {
            Released::End { end, position } => (end, position),
            Released::Call { parked, position } => {
                run_parked(build, &parked, position, 0);
                continue;
            }
        };
        let already_ran = RAN_ENDS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(&end.name, end.phase);
        if already_ran {
            log_line(&format!(
                "callback follow: scene follow, queued {} (queue position {position}) ran here already",
                end.name
            ));
            continue;
        }
        log_line(&format!(
            "callback follow: scene follow, running queued {} (queue position {position}) in the partner's order",
            end.name
        ));
        start(build, end.phase, &end.name, now);
    }
}

fn decide(name: &str, announce_when_alone: bool) -> bool {
    let Some(net) = net::handle() else {
        return false;
    };
    let view = net.view();
    let Some(role) = gate_role(&view) else {
        return false;
    };
    let Some(phase) = shared_phase(&view) else {
        return false;
    };
    let partner_place = view.partner_place();
    let led_by_partner = GATE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .partner_led(name, net_clock_ms());
    if own_scene_start_waits_for_the_partner(net, phase, name, partner_place, led_by_partner) {
        return true;
    }
    let verdict = {
        let mut gate = GATE.lock().unwrap_or_else(PoisonError::into_inner);
        gate.note_partner_here(partner_place.is_same());
        gate.local_fire(role, name, net_clock_ms())
    };
    let announce = || {
        net.send_relayed(Message::CallbackFired(CallbackFired {
            phase,
            name: name.to_owned(),
        }));
    };
    match (role, verdict) {
        (GateRole::Host, LocalFire::RunReleased) => {
            log_line(&format!(
                "callback follow: {name} starts from the guest's step, sent to the guest"
            ));
            announce();
        }
        (GateRole::Host, _) => {
            log_line(&format!(
                "callback follow: you fired {name}, sent to the guest"
            ));
            announce();
        }
        (GateRole::Guest, LocalFire::Hold) => {
            log_line(&format!(
                "callback follow: your game fired {name} before the host did, held; the host is asked to fire it"
            ));
            announce();
            return true;
        }
        (GateRole::Guest, LocalFire::RunAlone) => {
            log_line(&format!(
                "callback follow: your game fired {name} with the host {}, it runs here only",
                partner_place.describe()
            ));
            if announce_when_alone {
                announce();
            }
        }
        (GateRole::Guest, LocalFire::RunReleased) => {
            log_line(&format!("callback follow: {name} released, it runs here"));
        }
        (GateRole::Guest, _) => {
            log_line(&format!(
                "callback follow: the host fired {name} first, it runs here"
            ));
        }
    }
    false
}

pub(super) fn clear_per_phase_sets_after_a_load() {
    let loads = crate::player_body::phase_load_count();
    let (restarts, restart_why) = restart_signals();
    let cause = LOAD_WATCH
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear_cause(loads, restarts);
    let Some(cause) = cause else {
        return;
    };
    RAN_ENDS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .reset();
    SHARED_RAN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .reset();
    RELAYED_SCENES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .reset();
    STARTED_FROM_PARTNER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .reset();
    reset_shared_functions();
    reset_message_handlers();
    let why = match cause {
        ClearCause::PhaseLoad => "a phase was loaded",
        ClearCause::RestartSignal => restart_why.as_str(),
    };
    forget_relay_call_in_flight(why);
    log_line(&format!("per-phase sets cleared: {why}"));
}

pub(super) fn tick(build: &VerifiedBuild, net: &NetThread, view: &NetShared) {
    clear_per_phase_sets_after_a_load();
    let (ends, received): (Vec<Message>, Vec<Message>) = net
        .take_relayed(|message| {
            matches!(
                message,
                Message::CallbackFired(_)
                    | Message::SharedFunctionRan(_)
                    | Message::MessageHandlerRan(_)
            )
        })
        .into_iter()
        .partition(|message| match message {
            Message::CallbackFired(fired) => is_end_callback(&fired.name),
            _ => true,
        });
    let ends = drop_covered_by_the_fight_stream(ends);
    let received = drop_covered_by_the_fight_stream(received);
    let role = gate_role(view);
    let (Some(role), Some(phase)) = (role, shared_phase(view)) else {
        let now = net_clock_ms();
        let mut gate = GATE.lock().unwrap_or_else(PoisonError::into_inner);
        let released = gate.release_held(now);
        if view.state != NetState::Connected {
            gate.reset();
            let dropped_calls = WAITING_ENDS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clear_returning_calls();
            if dropped_calls > 0 {
                log_line(&format!(
                    "partner calls: {dropped_calls} waiting call(s) dropped, the session ended"
                ));
            }
            STARTED_FROM_PARTNER
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .reset();
            RAN_ENDS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .reset();
            SHARED_RAN
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .reset();
            RELAYED_SCENES
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .reset();
            *LATEST_PARTNER_END
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = None;
            reset_shared_functions();
            reset_message_handlers();
            drop_relay_calls("the session ended");
        }
        drop(gate);
        for message in ends {
            match message {
                Message::CallbackFired(fired) => log_line(&format!(
                    "callback follow: the partner ended {}, not run here: you are not in the same play",
                    fired.name
                )),
                Message::SharedFunctionRan(ran) => log_line(&format!(
                    "shared function: {} from the partner not run here: you are not in the same play",
                    ran.name
                )),
                Message::MessageHandlerRan(ran) => log_line(&format!(
                    "message handler: {} from the partner not run here: you are not in the same play",
                    ran.name
                )),
                _ => {}
            }
        }
        let phase = local_phase(build);
        for name in released {
            log_line(&format!(
                "callback follow: the host is no longer in the same play, held {name} runs here"
            ));
            if let Some(phase) = phase {
                start(build, phase, &name, now);
            }
        }
        return;
    };
    let local = local_phase(build);
    set_local_phase(local);
    relay_calls_enter_phase(local);
    for message in ends {
        match message {
            Message::CallbackFired(fired) => {
                *LATEST_PARTNER_END
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner) = Some(fired.name.clone());
                follow_end(build, role, view, &fired);
            }
            Message::SharedFunctionRan(ran) => receive_shared_function(build, role, &ran),
            Message::MessageHandlerRan(ran) => receive_message_handler(build, role, &ran),
            _ => {}
        }
    }
    run_waiting_ends(build, role, view);
    run_due_fill_ins(build, role);
    let now = net_clock_ms();
    let partner_place = view.partner_place();
    let mut gate = GATE.lock().unwrap_or_else(PoisonError::into_inner);
    gate.note_partner_here(partner_place.is_same());
    let mut releases = Vec::new();
    for message in received {
        let Message::CallbackFired(fired) = message else {
            continue;
        };
        if fired.phase != phase || Some(fired.phase) != local {
            log_line(&format!(
                "callback follow: {} fired in another phase on the partner's game, ignored",
                fired.name
            ));
            continue;
        }
        note_partner_ran_shared_event(fired.phase, &fired.name);
        match gate.partner_fire(role, &fired.name, now) {
            PartnerFire::Release { waited_ms } => {
                releases.push(fired.name.clone());
                log_line(&format!(
                    "callback follow: the host fired {}, your held copy starts now (held {waited_ms} ms)",
                    fired.name
                ));
            }
            PartnerFire::FollowHost if relayed_scene_played(phase, &fired.name) => {
                log_line(&format!(
                    "callback follow: the host started {}, its scene already played here from the story relay, not started again",
                    fired.name
                ));
            }
            PartnerFire::FollowHost => {
                releases.push(fired.name.clone());
                log_line(&format!(
                    "callback follow: the host started {} and you are in the same place, it starts here too",
                    fired.name
                ));
            }
            PartnerFire::Noted => log_line(&format!(
                "callback follow: the partner fired {}, your game has not yet",
                fired.name
            )),
            PartnerFire::HostWaits => log_line(&format!(
                "callback follow: the guest fired {} and your game has not, waiting {CALLBACK_GATE_HOST_GRACE_MS} ms for it",
                fired.name
            )),
            PartnerFire::Elsewhere => log_line(&format!(
                "callback follow: the guest fired {} {}, not followed here",
                fired.name,
                partner_place.describe()
            )),
            PartnerFire::HostFiredFirst => log_line(&format!(
                "callback follow: the guest fired {}, your game had fired it already",
                fired.name
            )),
        }
    }
    let steps = gate.step(role, main_menu_open(build), pause_menu_open(build), now);
    drop(gate);
    for name in releases {
        start(build, phase, &name, now);
    }
    for step in steps {
        match step {
            GateStep::GaveUp { name, waited_ms } => {
                log_line(&format!(
                    "callback follow: gave up waiting for the host to fire {name} after {waited_ms} ms, it runs here"
                ));
                if late_copy_verdict_here(phase, &name) == LateCopy::RanHereTraced {
                    log_line(&format!(
                        "callback follow: {name} was given up on, but it ran here already (traced), not started again"
                    ));
                    continue;
                }
                start(build, phase, &name, now);
            }
            GateStep::CloseDialog { name, waited_ms } => {
                log_line(&format!(
                    "callback follow: your game still has not fired {name} after {waited_ms} ms with a prompt open, closing it"
                ));
                close_main_menu(build, "to follow the guest's step");
            }
            GateStep::ClosePauseMenu { name, waited_ms } => {
                log_line(&format!(
                    "callback follow: your pause menu is open {waited_ms} ms after the guest fired {name}, closing it"
                ));
                close_pause_menu(build, "to follow the guest's step");
            }
            GateStep::Inject { name, waited_ms } => {
                if relayed_scene_played(phase, &name) {
                    log_line(&format!(
                        "callback follow: your game never fired {name} ({waited_ms} ms), its scene already played here from the story relay, not started"
                    ));
                    continue;
                }
                if crate::fight_sync::covers_phase(phase) {
                    log_line(&format!(
                        "callback follow: your game never fired {name} ({waited_ms} ms), not started: the fight stream decides inside the fight window"
                    ));
                    continue;
                }
                if late_copy_verdict_here(phase, &name) == LateCopy::RanHereTraced {
                    log_line(&format!(
                        "callback follow: your game never fired {name} ({waited_ms} ms), but it ran here already (traced), not started again"
                    ));
                    continue;
                }
                log_line(&format!(
                    "callback follow: your game never fired {name} ({waited_ms} ms), starting it from the guest's step"
                ));
                start(build, phase, &name, now);
            }
        }
    }
}

fn drop_covered_by_the_fight_stream(messages: Vec<Message>) -> Vec<Message> {
    messages
        .into_iter()
        .filter(|message| {
            let (phase, name) = match message {
                Message::CallbackFired(fired) => (fired.phase, fired.name.as_str()),
                Message::SharedFunctionRan(ran) => (ran.phase, ran.name.as_str()),
                Message::MessageHandlerRan(ran) => (ran.phase, ran.name.as_str()),
                _ => return true,
            };
            if !crate::fight_sync::covers_phase(phase) {
                return true;
            }
            log_line(&format!(
                "callback follow: the partner's {name} not run here, the fight stream covers this fight"
            ));
            false
        })
        .collect()
}

pub(super) fn start(build: &VerifiedBuild, phase: u16, name: &str, now: u64) {
    close_menus_for_modal_entry(build, phase, name);
    if is_end_callback(name) {
        let shared_event = phase_name(phase).is_some_and(|label| is_shared_event(label, name));
        crate::pickups::open_injected_end_window(name, shared_event);
        note_ran_end(name, phase);
    }
    note_injected(name, now);
    STARTED_FROM_PARTNER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .note(name, phase);
    let call = StoryCall {
        phase,
        pack: 0,
        function: name.to_owned(),
        args: Vec::new(),
    };
    let accepted = submit_relay_call(&call, RelayPath::CallbackFollow);
    log_line(&format!(
        "callback follow: {name} handed to the relay queue ({accepted})"
    ));
}

pub(super) fn gate_role(view: &NetShared) -> Option<GateRole> {
    if !view.partner_in_play() {
        return None;
    }
    match FollowRole::from_slot(view.local_slot) {
        FollowRole::Host => Some(GateRole::Host),
        FollowRole::Guest => Some(GateRole::Guest),
        FollowRole::Unknown => None,
    }
}

pub(super) fn shared_phase(view: &NetShared) -> Option<u16> {
    let local = view.local.as_ref()?.phase;
    let remote = view.partner_state()?.state.phase;
    (local == remote).then_some(local)
}
