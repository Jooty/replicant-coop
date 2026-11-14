use core::ffi::c_void;
use core::ptr;
use std::panic;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError};

use replicant_game::GameMemory;
use replicant_game::cutscene_skip::TRIGGER_MANAGER_RVA;
use replicant_game::fight_sync::{SlotState, script_slot_state};
use replicant_game::marker::CONTROL_MASK_FIRST_RVA;
use replicant_game::script_runner::{
    HostLedRuns, PAUSE_OFF_NATIVE_RVA, PAUSE_ON_NATIVE_RVA, PauseCall, QUEUE_FIRST_SLOT_OFFSET,
    QUEUE_SLOT_COUNT, QUEUE_SLOT_STRIDE, ScriptPauseWatch, ScriptRunnerLogGate,
    ScriptRunnerSnapshot, SlotHolderWatch, format_empty_pop, format_pause_call,
    format_runner_state, format_slot_holder,
};
use replicant_game::story_relay::{QUEUED_CALL_FREE_STATE, QUEUED_CALL_READY_STATE};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::fight_sync::script_manager_state;
use crate::game_hooks::menu_no_pause;
use crate::memory::InProcessMemory;
use crate::net::net_clock_ms;
use crate::stay_together::{main_menu_open, pause_menu_open};
use nc_hook::detour::hook_address;

static HOOKED_BUILD: OnceLock<VerifiedBuild> = OnceLock::new();
static ORIGINAL_PAUSE_ON: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static ORIGINAL_PAUSE_OFF: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static GATE: Mutex<ScriptRunnerLogGate> = Mutex::new(ScriptRunnerLogGate::new());
static LAST_POP: Mutex<Option<(String, u64)>> = Mutex::new(None);
static SLOT_HOLDER: Mutex<SlotHolderWatch> = Mutex::new(SlotHolderWatch::new());
static SCRIPT_PAUSES: Mutex<ScriptPauseWatch> = Mutex::new(ScriptPauseWatch::new());
static HOST_LED_RUNS: Mutex<HostLedRuns> = Mutex::new(HostLedRuns::new());

type LuaNative = unsafe extern "system" fn(*mut c_void);

pub(super) fn install(build: VerifiedBuild) {
    let _ = HOOKED_BUILD.set(build);
    let base = build.module_base();
    #[expect(unsafe_code, reason = "hooks the pause natives by address")]
    // SAFETY: each RVA starts the Lua native of that name in the verified build (lua state in,
    // nothing out), and the replacements have the same signature and call the original.
    unsafe {
        hook_address(
            "PauseOn_native",
            (base + u64::from(PAUSE_ON_NATIVE_RVA)) as usize,
            pause_on_detour as *const () as *mut c_void,
            &ORIGINAL_PAUSE_ON,
        );
        hook_address(
            "PauseOff_native",
            (base + u64::from(PAUSE_OFF_NATIVE_RVA)) as usize,
            pause_off_detour as *const () as *mut c_void,
            &ORIGINAL_PAUSE_OFF,
        );
    }
}

pub(super) fn note_pop(name: &str) {
    let now = net_clock_ms();
    *LAST_POP.lock().unwrap_or_else(PoisonError::into_inner) = Some((name.to_owned(), now));
    SLOT_HOLDER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .popped(name, now);
    HOST_LED_RUNS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .holder_popped(name);
}

/// The host's game led this handler here, arriving at `arrived_ms` (now if unknown); its next run is host-led.
pub(crate) fn note_host_led_handler(name: &str, arrived_ms: Option<u64>) {
    HOST_LED_RUNS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .announce(name, arrived_ms.unwrap_or_else(net_clock_ms));
}

pub(crate) fn script_pause_in_force() -> bool {
    SCRIPT_PAUSES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .in_force(net_clock_ms())
}

pub(crate) fn host_led_run() -> Option<(String, u64)> {
    let runs = HOST_LED_RUNS.lock().unwrap_or_else(PoisonError::into_inner);
    Some((runs.running_name()?.to_owned(), runs.running_since_ms()?))
}

pub(crate) fn host_led_runs_enter_phase(phase: Option<u16>) {
    HOST_LED_RUNS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .enter_phase(phase);
}

/// A hooked script native returned; the handler holding the script slot called it.
pub(crate) fn note_native(native: &'static str) {
    SLOT_HOLDER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .native_called(native, net_clock_ms());
}

/// A hooked native that holds the script step while it waits was called by the slot's holder.
pub(crate) fn note_yielding_native(native: &'static str) {
    SLOT_HOLDER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .yielding_native_called(native, net_clock_ms());
}

/// The handler holding the script slot now and for how many ms.
pub(crate) fn slot_holding_for() -> Option<(String, u64)> {
    SLOT_HOLDER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .holding_for(net_clock_ms())
        .map(|(name, in_slot_ms)| (name.to_owned(), in_slot_ms))
}

/// The last hooked native the slot's holder waited in, with how many ms ago.
pub(crate) fn last_yielding_native() -> Option<(&'static str, u64)> {
    SLOT_HOLDER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .last_yielding_native(net_clock_ms())
}

pub(crate) fn slot_holder_name() -> Option<String> {
    SLOT_HOLDER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .holding()
        .map(str::to_owned)
}

/// The last hooked native the last popped handler called, with how many ms ago.
pub(crate) fn last_native() -> Option<(&'static str, u64)> {
    SLOT_HOLDER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .last_native(net_clock_ms())
}

pub(crate) fn last_pop() -> Option<(String, u64)> {
    LAST_POP
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

pub(super) fn note_empty_pop() {
    let now = net_clock_ms();
    if !GATE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .empty_pop_due(now)
    {
        return;
    }
    let Some(snapshot) = read_snapshot(now) else {
        return;
    };
    if snapshot.queued > 0 {
        log_line(&format_empty_pop(&snapshot));
    }
}

pub fn log_runner_state() {
    let now = net_clock_ms();
    log_slot_holder(now);
    let Some(snapshot) = read_snapshot(now) else {
        return;
    };
    if !GATE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .state_due(snapshot.is_idle(), now)
    {
        return;
    }
    log_line(&format_runner_state(&snapshot));
}

fn log_slot_holder(now_ms: u64) {
    let Some(build) = HOOKED_BUILD.get() else {
        return;
    };
    let slot = script_slot_state(script_manager_state(build));
    if slot == SlotState::Idle {
        HOST_LED_RUNS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .slot_idle();
    }
    let report = SLOT_HOLDER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .tick(slot, now_ms);
    let Some(report) = report else {
        return;
    };
    let pause_mask = read_pause_mask(build.module_base(), &InProcessMemory::new(build));
    let menu_open = main_menu_open(build) || pause_menu_open(build);
    log_line(&format_slot_holder(&report, pause_mask, menu_open));
}

fn read_snapshot(now_ms: u64) -> Option<ScriptRunnerSnapshot> {
    let build = HOOKED_BUILD.get()?;
    let memory = InProcessMemory::new(build);
    let base = build.module_base();
    let pause_mask = read_pause_mask(base, &memory);
    let queue_start = base + u64::from(TRIGGER_MANAGER_RVA) + QUEUE_FIRST_SLOT_OFFSET;
    let mut queued = 0;
    let mut ready = 0;
    for slot in 0..QUEUE_SLOT_COUNT {
        let state = memory
            .read_i32(queue_start + slot * QUEUE_SLOT_STRIDE)
            .unwrap_or(QUEUED_CALL_FREE_STATE);
        if state != QUEUED_CALL_FREE_STATE {
            queued += 1;
        }
        if state == QUEUED_CALL_READY_STATE {
            ready += 1;
        }
    }
    let last_pop = LAST_POP
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map(|(name, at)| (name.clone(), now_ms.saturating_sub(*at)));
    let manager_state = script_manager_state(build);
    let last_native = SLOT_HOLDER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .last_native(now_ms);
    Some(ScriptRunnerSnapshot {
        pause_mask,
        queued,
        ready,
        last_pop,
        slot: script_slot_state(manager_state),
        manager_state,
        last_native,
    })
}

fn read_pause_mask(base: u64, memory: &InProcessMemory) -> u32 {
    memory
        .read_i32(base + u64::from(CONTROL_MASK_FIRST_RVA))
        .unwrap_or(0)
        .cast_unsigned()
}

extern "system" fn pause_on_detour(lua_state: *mut c_void) {
    run_pause_native(&ORIGINAL_PAUSE_ON, PauseCall::On, lua_state);
}

extern "system" fn pause_off_detour(lua_state: *mut c_void) {
    run_pause_native(&ORIGINAL_PAUSE_OFF, PauseCall::Off, lua_state);
}

#[expect(
    unsafe_code,
    reason = "calls the original Lua native through the trampoline MinHook built"
)]
fn run_pause_native(original: &AtomicPtr<c_void>, call: PauseCall, lua_state: *mut c_void) {
    let original = original.load(Ordering::Acquire);
    if original.is_null() {
        return;
    }
    let before = panic::catch_unwind(current_pause_mask).unwrap_or(0);
    menu_no_pause::note_slot_zero_writer();
    // SAFETY: the trampoline continues into the original native, which has this exact signature.
    unsafe { core::mem::transmute::<*mut c_void, LuaNative>(original)(lua_state) };
    let native = match call {
        PauseCall::On => "_PauseOn",
        PauseCall::Off => "_PauseOff",
    };
    let _ = panic::catch_unwind(|| {
        note_script_pause_call(call);
        note_native(native);
        log_pause_call(call, before);
    });
}

fn note_script_pause_call(call: PauseCall) {
    let mut pauses = SCRIPT_PAUSES.lock().unwrap_or_else(PoisonError::into_inner);
    match call {
        PauseCall::On => pauses.pause_on_called(net_clock_ms()),
        PauseCall::Off => pauses.pause_off_called(),
    }
}

fn current_pause_mask() -> u32 {
    HOOKED_BUILD.get().map_or(0, |build| {
        read_pause_mask(build.module_base(), &InProcessMemory::new(build))
    })
}

fn log_pause_call(call: PauseCall, before: u32) {
    let running = LAST_POP
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map(|(name, _)| name.clone())
        .unwrap_or_default();
    log_line(&format_pause_call(
        call,
        before,
        current_pause_mask(),
        &running,
    ));
}
