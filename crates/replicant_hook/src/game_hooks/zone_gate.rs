use core::ffi::{CStr, c_void};
use core::ptr;
use std::collections::HashSet;
use std::panic;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock, PoisonError};

use replicant_game::GameMemory;
use replicant_game::lift_rides::warp_pick_origin;
use replicant_game::offsets::{
    CHANGE_MAP_NATIVE_FUNCTION_RVA, CHANGE_PHASE_NATIVE_FUNCTION_RVA,
    POP_FIRED_CALLBACK_FUNCTION_RVA, SELECT_WARP_POINT_NATIVE_FUNCTION_RVA,
    TRIGGER_CALLBACK_NAME_CAPACITY, WARP_SELECT_ORIGIN_POINT_RVA,
};
use replicant_game::zone_rules::{ZoneChangeSource, is_zone_exit_callback};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::fight_sync::FightPop;
use crate::game_hooks::door_walk::take_recent_door_use;
use crate::game_hooks::script_runner_log;
use crate::game_hooks::side_view_log;
use crate::game_hooks::teleport::request_teleport;
use crate::memory::InProcessMemory;
use crate::story_relay::suppress_fired_call;
use crate::zone_rules::veto_zone_change;
use nc_hook::detour::hook_address;
use nc_hook::stall_watch::in_game_thread_step;

const MAX_SEEN_CALLBACK_NAMES: usize = 400;

static SEEN_CALLBACKS: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Mutex::default);
static ORIGINAL_POP_FIRED_CALLBACK: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static ORIGINAL_CHANGE_MAP: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static ORIGINAL_CHANGE_PHASE: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static ORIGINAL_SELECT_WARP_POINT: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static HOOKED_BUILD: OnceLock<VerifiedBuild> = OnceLock::new();

type PopFiredCallback = unsafe extern "system" fn(*mut c_void, *mut u8, *mut *mut c_void) -> u64;
type SelectWarpPoint = unsafe extern "system" fn(i32, i32);
type LuaNative = unsafe extern "system" fn(*mut c_void);

pub(super) fn install(build: VerifiedBuild) {
    side_view_log::install(build);
    script_runner_log::install(build);
    let _ = HOOKED_BUILD.set(build);
    let base = build.module_base();
    #[expect(unsafe_code, reason = "hooks the zone change entry points by address")]
    // SAFETY: each RVA starts the function the replacement mirrors in the verified build:
    // TriggerManager pop of a fired callback, and the _ChangeMap and _ChangePhase Lua natives.
    unsafe {
        hook_address(
            "TriggerManager_PopFiredCallback",
            (base + u64::from(POP_FIRED_CALLBACK_FUNCTION_RVA)) as usize,
            pop_fired_callback_detour as *const () as *mut c_void,
            &ORIGINAL_POP_FIRED_CALLBACK,
        );
        hook_address(
            "ChangeMap_native",
            (base + u64::from(CHANGE_MAP_NATIVE_FUNCTION_RVA)) as usize,
            change_map_detour as *const () as *mut c_void,
            &ORIGINAL_CHANGE_MAP,
        );
        hook_address(
            "ChangePhase_native",
            (base + u64::from(CHANGE_PHASE_NATIVE_FUNCTION_RVA)) as usize,
            change_phase_detour as *const () as *mut c_void,
            &ORIGINAL_CHANGE_PHASE,
        );
        hook_address(
            "SelectWarpPoint_native",
            (base + u64::from(SELECT_WARP_POINT_NATIVE_FUNCTION_RVA)) as usize,
            select_warp_point_detour as *const () as *mut c_void,
            &ORIGINAL_SELECT_WARP_POINT,
        );
    }
}

#[expect(
    unsafe_code,
    reason = "calls the original pop function and reads the name buffer it filled"
)]
extern "system" fn pop_fired_callback_detour(
    manager: *mut c_void,
    name_out: *mut u8,
    argument_out: *mut *mut c_void,
) -> u64 {
    let original = ORIGINAL_POP_FIRED_CALLBACK.load(Ordering::Acquire);
    if original.is_null() {
        return 0;
    }
    crate::fight_sync::before_pop(manager as u64);
    // SAFETY: the trampoline continues into the original function with this exact signature.
    let fired = unsafe {
        core::mem::transmute::<*mut c_void, PopFiredCallback>(original)(
            manager,
            name_out,
            argument_out,
        )
    };
    panic::catch_unwind(panic::AssertUnwindSafe(|| {
        in_game_thread_step("script pop: judging a popped callback", || {
            judge_popped_callback(manager as u64, fired, name_out, argument_out)
        })
    }))
    .unwrap_or_else(|_| {
        log_line(
            "zone rules: panic while judging a popped callback, it runs as the game popped it",
        );
        fired
    })
}

#[expect(
    unsafe_code,
    reason = "reads the name buffer and argument pointer the original pop function filled"
)]
fn judge_popped_callback(
    manager: u64,
    fired: u64,
    name_out: *mut u8,
    argument_out: *mut *mut c_void,
) -> u64 {
    side_view_log::observe();
    if fired & 0xFF == 0 || name_out.is_null() {
        script_runner_log::note_empty_pop();
        return fired;
    }
    // SAFETY: the original just wrote a NUL-terminated name into the caller's 0x40 byte buffer.
    let name = unsafe { callback_name(name_out) };
    note_callback_seen(&name);
    side_view_log::note_callback(&name);
    script_runner_log::note_pop(&name);
    // SAFETY: the original just stored the argument block pointer through this out pointer.
    let arguments = if argument_out.is_null() {
        0
    } else {
        unsafe { *argument_out as u64 }
    };
    let queued_by_the_mod = crate::story_relay::relay_call_popped(arguments);
    if !queued_by_the_mod && crate::cutscene_skip::drop_start_of_a_played_scene(&name) {
        return 0;
    }
    if let Some(build) = HOOKED_BUILD.get() {
        crate::kaine::companion_off_function_ran(build, &name);
    }
    crate::cutscene_skip::note_fired_callback(&name);
    match crate::fight_sync::judge_pop(manager, &name, arguments) {
        FightPop::Hold => return 0,
        FightPop::Replayed => {
            crate::one_sided::note_fired(&name);
            crate::kaine::stand_in_function_ran(&name);
            return fired;
        }
        FightPop::HostWindow => return run_inside_the_fight_window(fired, &name, arguments),
        FightPop::Outside => {}
    }
    if crate::cutscene_skip::is_injected_duplicate(&name) {
        return 0;
    }
    if !queued_by_the_mod
        && let Some(build) = HOOKED_BUILD.get()
        && crate::kaine::hold_own_stand_in_trap(build, &name, arguments)
    {
        return 0;
    }
    if crate::cutscene_skip::hold_fired_callback(&name) {
        return 0;
    }
    if is_zone_exit_callback(&name)
        && vetoed(
            &format!("exit callback {name}"),
            ZoneChangeSource::PlayerChosen,
        )
    {
        put_player_back_at_the_door();
        return 0;
    }
    if suppress_fired_call(&name, arguments) {
        return 0;
    }
    crate::one_sided::note_fired(&name);
    crate::kaine::stand_in_function_ran(&name);
    crate::fight_sync::pop_ran(&name, arguments);
    fired
}

fn run_inside_the_fight_window(fired: u64, name: &str, arguments: u64) -> u64 {
    if is_zone_exit_callback(name)
        && vetoed(
            &format!("exit callback {name}"),
            ZoneChangeSource::PlayerChosen,
        )
    {
        put_player_back_at_the_door();
        return 0;
    }
    crate::one_sided::note_fired(name);
    crate::kaine::stand_in_function_ran(name);
    crate::fight_sync::pop_ran(name, arguments);
    fired
}

#[expect(unsafe_code, reason = "reads the callback name buffer")]
unsafe fn callback_name(name_out: *const u8) -> String {
    // SAFETY: the buffer holds TRIGGER_CALLBACK_NAME_CAPACITY bytes and ends in NUL (strcpy_s).
    let bytes = unsafe { core::slice::from_raw_parts(name_out, TRIGGER_CALLBACK_NAME_CAPACITY) };
    CStr::from_bytes_until_nul(bytes)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn put_player_back_at_the_door() {
    let Some(position) = take_recent_door_use() else {
        return;
    };
    request_teleport(position);
    log_line(&format!(
        "zone gate: refused after the walk, moved back to {:.1}, {:.1}, {:.1}",
        position.x, position.y, position.z
    ));
}

fn vetoed(path: &str, source: ZoneChangeSource) -> bool {
    panic::catch_unwind(|| veto_zone_change(path, source)).unwrap_or(false)
}

extern "system" fn change_map_detour(lua_state: *mut c_void) {
    run_native_unless_vetoed(&ORIGINAL_CHANGE_MAP, "_ChangeMap native", lua_state);
}

extern "system" fn change_phase_detour(lua_state: *mut c_void) {
    run_native_unless_vetoed(&ORIGINAL_CHANGE_PHASE, "_ChangePhase native", lua_state);
}

#[expect(
    unsafe_code,
    reason = "calls the original Lua native through the trampoline MinHook built"
)]
fn run_native_unless_vetoed(original: &AtomicPtr<c_void>, path: &str, lua_state: *mut c_void) {
    let original = original.load(Ordering::Acquire);
    if original.is_null() || vetoed(path, ZoneChangeSource::Scripted) {
        return;
    }
    // SAFETY: the trampoline continues into the original native, which has this exact signature.
    unsafe { core::mem::transmute::<*mut c_void, LuaNative>(original)(lua_state) };
}

#[expect(
    unsafe_code,
    reason = "calls the original warp selection through the trampoline MinHook built"
)]
extern "system" fn select_warp_point_detour(point: i32, other: i32) {
    let original = ORIGINAL_SELECT_WARP_POINT.load(Ordering::Acquire);
    if original.is_null() || guest_called_a_lift(point, other) {
        return;
    }
    if vetoed("warp menu", ZoneChangeSource::PlayerChosen) {
        return;
    }
    log_line(&format!(
        "zone rules: SelectWarpPoint(point {point}, other {other}) passed"
    ));
    // SAFETY: the trampoline continues into the original function, which takes these two ints.
    unsafe { core::mem::transmute::<*mut c_void, SelectWarpPoint>(original)(point, other) };
}

fn guest_called_a_lift(point: i32, other: i32) -> bool {
    let Some(build) = HOOKED_BUILD.get() else {
        return false;
    };
    panic::catch_unwind(|| {
        let window_origin = InProcessMemory::new(build)
            .read_i32(build.module_base() + u64::from(WARP_SELECT_ORIGIN_POINT_RVA))
            .unwrap_or(-1);
        let origin = warp_pick_origin(other, window_origin);
        crate::lift_rides::guest_calls_the_lift(build, point, origin)
    })
    .unwrap_or(false)
}

#[expect(
    unsafe_code,
    reason = "calls the original warp selection through the trampoline MinHook built"
)]
pub(crate) fn run_warp_pick(point: i32, origin: i32) -> bool {
    // Skips the zone rules on purpose: the host runs a lift pick the guest made.
    let original = ORIGINAL_SELECT_WARP_POINT.load(Ordering::Acquire);
    if original.is_null() {
        return false;
    }
    // SAFETY: the trampoline continues into the original function, which takes these two ints;
    // it only queues the script's pick handler, so it is safe on the game thread at any time.
    unsafe { core::mem::transmute::<*mut c_void, SelectWarpPoint>(original)(point, origin) };
    true
}

fn note_callback_seen(name: &str) {
    let mut seen = SEEN_CALLBACKS
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if seen.len() < MAX_SEEN_CALLBACK_NAMES && seen.insert(name.to_owned()) {
        log_line(&format!("zone rules: callback fired: {name}"));
    }
}
