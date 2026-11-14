use std::collections::BTreeSet;
use std::sync::{Mutex, PoisonError};

use replicant_game::callback_gate::{
    GateRole, ParkedCall, ParkedKind, is_end_callback, is_gated_callback,
};
use replicant_game::readiness::{classify, read_play_checks};
use replicant_game::script_functions::{
    DrainStep, FILL_IN_GRACE_MS, FillInArrival, FillInCopy, FillInReach, FillInRelay,
    FunctionArrival, FunctionArrivalFacts, FunctionsRan, LateCopy, TracedRelay, fill_in_reach,
    function_arrival, is_restart_function, judge_function, late_copy_verdict, plan_forced_drain,
    script_function, shows_modal_ui, traced_relay,
};
use replicant_game::shared_events::phase_name;
use replicant_game::{PhaseEntry, Replicant};
use replicant_proto::{Message, SharedFunctionRan, StoryCall};

use crate::build_gate::VerifiedBuild;
use crate::cutscene_skip::callback_gate::{
    WAITING_ENDS, allow_next, clear_per_phase_sets_after_a_load, run_released, start,
};
use crate::cutscene_skip::restart_signals::note_restart;
use crate::cutscene_skip::scene_follow::note_injected;
use crate::diagnostics::log_line;
use crate::memory::InProcessMemory;
use crate::net::NetSharedExt;
use crate::net::{self, NetState, net_clock_ms};
use crate::stay_together::{close_main_menu, close_pause_menu, main_menu_open, pause_menu_open};
use crate::story_relay::{
    RelayPath, is_message_handler, relay_calls_are_clear, shared_story_events, submit_relay_call,
};

const POPPED_CALLBACK_MATCH_MS: u64 = 5_000;
const INJECTED_FUNCTION_PENDING_MS: u64 = 20_000;
const MAX_PENDING_INJECTED_FUNCTIONS: usize = 32;

static RAN: Mutex<FunctionsRan> = Mutex::new(FunctionsRan::new());
static INJECTED: Mutex<Vec<(String, u64)>> = Mutex::new(Vec::new());
static LAST_POPPED: Mutex<Option<(String, u64)>> = Mutex::new(None);
static MISSING_FROM_TABLE_LOGGED: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
static FILL_IN: Mutex<FillInRelay> = Mutex::new(FillInRelay::new());

pub(super) fn note_popped_callback(name: &str) {
    *LAST_POPPED.lock().unwrap_or_else(PoisonError::into_inner) =
        Some((name.to_owned(), net_clock_ms()));
}

pub(super) fn set_local_phase(phase: Option<u16>) {
    RAN.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .enter_phase(phase);
    let dropped = FILL_IN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .enter_phase(phase);
    if dropped > 0 {
        log_line(&format!(
            "shared function: {dropped} waiting partner copy(ies) dropped, you are in another phase now"
        ));
    }
}

pub(super) fn reset_shared_functions() {
    RAN.lock().unwrap_or_else(PoisonError::into_inner).reset();
    INJECTED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    *LAST_POPPED.lock().unwrap_or_else(PoisonError::into_inner) = None;
    let dropped = FILL_IN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    if dropped > 0 {
        log_line(&format!(
            "shared function: {dropped} waiting partner copy(ies) dropped with the per-phase sets"
        ));
    }
}

pub(crate) fn on_function_traced(build: &VerifiedBuild, name: &str) {
    if is_restart_function(name) {
        note_restart(&format!("{name} ran"));
    }
    clear_per_phase_sets_after_a_load();
    crate::kaine::companion_off_function_ran(build, name);
    crate::kaine::stand_in_function_ran(name);
    let Some((phase, label)) = local_phase_and_label(build) else {
        return;
    };
    crate::fight_sync::function_traced(name);
    let popped = take_popped_callback(name);
    let relay = traced_relay(
        &label,
        name,
        popped,
        popped && is_message_handler(phase, name),
    );
    if relay == TracedRelay::NotRelayed {
        return;
    }
    let newly = RAN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .note(phase, name);
    log_line(&format!("trace: {name} ({label})"));
    if take_pending_injected(name) {
        log_line(&format!(
            "shared function: {name} ran here from the partner's call"
        ));
        return;
    }
    let replaced = FILL_IN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .ran_here(phase, name);
    if let Some(copy) = replaced {
        log_line(&format!(
            "shared function: {name} ran here on its own {} ms after the partner's copy arrived, the partner's copy is dropped",
            net_clock_ms().saturating_sub(copy.arrived_ms)
        ));
    }
    if !newly {
        return;
    }
    if crate::fight_sync::covers_phase(phase) {
        log_line(&format!(
            "shared function: {name} is inside the fight window, the fight stream relays it"
        ));
        return;
    }
    if relay == TracedRelay::CallbackPathRelays {
        log_line(&format!(
            "shared function: {name} was a popped callback, the callback path relays it"
        ));
        return;
    }
    let Some(net) = net::handle() else {
        return;
    };
    let view = net.view();
    let partner_same_phase = view.partner_in_play()
        && view
            .partner_state()
            .is_some_and(|remote| remote.state.phase == phase);
    if !shared_story_events() || view.state != NetState::Connected || !partner_same_phase {
        log_line(&format!(
            "shared function: {name} ran here only, not sent (the partner is not in the same place)"
        ));
        return;
    }
    net.send_relayed(Message::SharedFunctionRan(SharedFunctionRan {
        phase,
        name: name.to_owned(),
    }));
    log_line(&format!("shared function: {name} sent"));
}

pub(super) fn receive_shared_function(
    build: &VerifiedBuild,
    role: GateRole,
    ran: &SharedFunctionRan,
) {
    let partner = partner_of(role);
    let (verdict, label) = arrival_verdict(build, ran.phase, &ran.name);
    if let FunctionArrival::Drop(reason) = verdict {
        log_line(&format!(
            "shared function: {} from the {partner} dropped ({})",
            ran.name,
            reason.describe()
        ));
        return;
    }
    let reach = fill_in_reach(label, &ran.name);
    let arrival = FILL_IN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .arrive(ran.phase, &ran.name, reach, net_clock_ms());
    match (arrival, reach) {
        (FillInArrival::AlreadyWaiting, _) => log_line(&format!(
            "shared function: {} from the {partner} is already waiting here",
            ran.name
        )),
        (FillInArrival::Waiting { position, .. }, FillInReach::OwnCopyExpected) => {
            log_line(&format!(
                "shared function: {} from the {partner} waits up to {FILL_IN_GRACE_MS} ms for your game's own copy (fill-in position {position})",
                ran.name
            ));
        }
        (FillInArrival::Waiting { position, .. }, FillInReach::CannotReachHere) => {
            log_line(&format!(
                "shared function: {} from the {partner} runs when its turn comes, your game cannot reach it on its own (fill-in position {position})",
                ran.name
            ));
        }
    }
}

pub(super) fn run_due_fill_ins(build: &VerifiedBuild, role: GateRole) {
    let now = net_clock_ms();
    let due = FILL_IN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .tick(relay_calls_are_clear(), now);
    if let Some(copy) = due {
        run_fill_in(build, partner_of(role), &copy, now);
    }
}

fn run_fill_in(build: &VerifiedBuild, partner: &str, copy: &FillInCopy, now: u64) {
    let name = copy.name.as_str();
    let why = match copy.reach {
        FillInReach::OwnCopyExpected => format!(
            "your game did not run it itself within {} ms",
            now.saturating_sub(copy.arrived_ms)
        ),
        FillInReach::CannotReachHere => "your game cannot reach it on its own".to_owned(),
    };
    let (verdict, label) = arrival_verdict(build, copy.phase, name);
    let call = StoryCall {
        phase: copy.phase,
        pack: 0,
        function: copy.name.clone(),
        args: Vec::new(),
    };
    match verdict {
        FunctionArrival::Drop(reason) => log_line(&format!(
            "shared function: {name} from the {partner} dropped when its turn came ({})",
            reason.describe()
        )),
        FunctionArrival::RunNow => {
            log_line(&format!(
                "shared function: {name} from the {partner} runs now ({why})"
            ));
            queue_shared_function(build, &call);
        }
        FunctionArrival::Park => {
            let parked = ParkedCall {
                kind: ParkedKind::Function,
                call,
            };
            let position = WAITING_ENDS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .park_call_always(parked, now);
            log_line(&format!(
                "shared function: {name} from the {partner} queued at position {position:?} behind earlier partner calls ({why})"
            ));
        }
        FunctionArrival::ForcedDrain => {
            log_line(&format!(
                "shared function: {name} from the {partner} starts its scene here ({why})"
            ));
            forced_drain(build, partner, label, call, ParkedKind::Function);
        }
    }
}

fn arrival_verdict(
    build: &VerifiedBuild,
    phase: u16,
    name: &str,
) -> (FunctionArrival, &'static str) {
    let game = Replicant::new(InProcessMemory::new(build));
    let local_in_play = classify(read_play_checks(&game)).is_in_play();
    let local_phase = current_phase_index(&game);
    let label = phase_name(phase).unwrap_or_default();
    let (queue_is_empty, queued) = {
        let queue = WAITING_ENDS.lock().unwrap_or_else(PoisonError::into_inner);
        (queue.is_empty(), queue.queued_function_names(phase))
    };
    let facts = FunctionArrivalFacts {
        enabled: shared_story_events(),
        local_in_play,
        local_phase,
        call_phase: phase,
        phase_label: label,
        queue_is_empty,
    };
    let ran_here = RAN.lock().unwrap_or_else(PoisonError::into_inner);
    (function_arrival(&facts, name, &ran_here, &queued), label)
}

const fn partner_of(role: GateRole) -> &'static str {
    match role {
        GateRole::Host => "guest",
        GateRole::Guest => "host",
    }
}

pub(super) fn forced_drain(
    build: &VerifiedBuild,
    partner: &str,
    label: &str,
    call: StoryCall,
    kind: ParkedKind,
) {
    let name = call.function.clone();
    let taken = {
        let mut queue = WAITING_ENDS.lock().unwrap_or_else(PoisonError::into_inner);
        queue.park_call_always(ParkedCall { kind, call }, net_clock_ms());
        if kind == ParkedKind::Handler {
            queue.take_through_handler(&name)
        } else {
            queue.take_through_function(&name)
        }
    };
    log_line(&format!(
        "forced drain for {name}: the {partner} started a scene, {} queued partner call(s) run first in order",
        taken.len().saturating_sub(1)
    ));
    close_menus_for_modal_entry_by_label(build, label, &name, "the partner's scene starts");
    for step in plan_forced_drain(taken, &name, label) {
        match step {
            DrainStep::Run(released) => run_released(build, vec![released]),
            DrainStep::Superseded { parked, position } => log_line(&format!(
                "forced drain for {name}: {} (queue position {position}) superseded by {name}",
                parked.call.function
            )),
        }
    }
}

pub(crate) fn run_shared_function(build: &VerifiedBuild, call: &StoryCall) {
    queue_shared_function(build, call);
}

fn queue_shared_function(build: &VerifiedBuild, call: &StoryCall) {
    let name = call.function.as_str();
    let label = phase_name(call.phase).unwrap_or_default();
    let queued = WAITING_ENDS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .queued_function_names(call.phase);
    let dropped = {
        let ran = RAN.lock().unwrap_or_else(PoisonError::into_inner);
        judge_function(label, call.phase, name, &ran, &queued)
    };
    if let Some(reason) = dropped {
        log_line(&format!(
            "shared function: {name} dropped when its turn came ({})",
            reason.describe()
        ));
        return;
    }
    let now = net_clock_ms();
    RAN.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .note(call.phase, name);
    remember_injected(name, now);
    if is_end_callback(name) {
        start(build, call.phase, name, now);
        log_line(&format!("shared function: {name} ran here as an end"));
        return;
    }
    close_menus_for_modal_entry_by_label(build, label, name, "a partner call shows UI");
    note_injected(name, now);
    if is_gated_callback(name) {
        allow_next(name);
    }
    let accepted = submit_relay_call(call, RelayPath::SharedFunction);
    log_line(&format!(
        "shared function: {name} runs here from the partner's call (handed to the relay queue {accepted})"
    ));
}

pub(super) fn note_run_from_partner(phase: u16, name: &str, now: u64) {
    RAN.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .note(phase, name);
    remember_injected(name, now);
}

pub(super) fn late_copy_verdict_here(phase: u16, name: &str) -> LateCopy {
    let ran = RAN.lock().unwrap_or_else(PoisonError::into_inner);
    late_copy_verdict(&ran, phase, name)
}

pub(crate) fn close_menus_for_modal_entry(build: &VerifiedBuild, phase: u16, name: &str) {
    let label = phase_name(phase).unwrap_or_default();
    close_menus_for_modal_entry_by_label(build, label, name, "a partner entry shows UI");
}

fn close_menus_for_modal_entry_by_label(build: &VerifiedBuild, label: &str, name: &str, why: &str) {
    if !shows_modal_ui(label, name) {
        if script_function(label, name).is_none() {
            note_function_missing_from_table(name);
        }
        return;
    }
    if main_menu_open(build) {
        close_main_menu(build, why);
    }
    if pause_menu_open(build) {
        log_line(&format!(
            "shared function: {name} shows UI, closing your pause menu first"
        ));
        close_pause_menu(build, why);
    }
}

fn note_function_missing_from_table(name: &str) {
    let first_time = MISSING_FROM_TABLE_LOGGED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(name.to_owned());
    if first_time {
        log_line(&format!(
            "shared function: {name} is not in the function table"
        ));
    }
}

fn remember_injected(name: &str, now: u64) {
    let mut injected = INJECTED.lock().unwrap_or_else(PoisonError::into_inner);
    injected.retain(|(known, at)| {
        known != name && now.saturating_sub(*at) <= INJECTED_FUNCTION_PENDING_MS
    });
    if injected.len() >= MAX_PENDING_INJECTED_FUNCTIONS {
        injected.remove(0);
    }
    injected.push((name.to_owned(), now));
}

fn take_pending_injected(name: &str) -> bool {
    let now = net_clock_ms();
    let mut injected = INJECTED.lock().unwrap_or_else(PoisonError::into_inner);
    injected.retain(|(_, at)| now.saturating_sub(*at) <= INJECTED_FUNCTION_PENDING_MS);
    let Some(index) = injected.iter().position(|(known, _)| known == name) else {
        return false;
    };
    injected.remove(index);
    true
}

fn take_popped_callback(name: &str) -> bool {
    let mut last = LAST_POPPED.lock().unwrap_or_else(PoisonError::into_inner);
    let matches = last.as_ref().is_some_and(|(popped_name, at)| {
        popped_name == name && net_clock_ms().saturating_sub(*at) <= POPPED_CALLBACK_MATCH_MS
    });
    if matches {
        *last = None;
    }
    matches
}

fn local_phase_and_label(build: &VerifiedBuild) -> Option<(u16, String)> {
    let game = Replicant::new(InProcessMemory::new(build));
    let label = game.current_phase().ok()?;
    let phase = u16::try_from(PhaseEntry::index_of(&label)?).ok()?;
    Some((phase, label))
}

fn current_phase_index(game: &Replicant<InProcessMemory>) -> Option<u16> {
    game.current_phase()
        .ok()
        .as_deref()
        .and_then(PhaseEntry::index_of)
        .and_then(|index| u16::try_from(index).ok())
}
