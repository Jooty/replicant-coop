use core::ffi::c_void;
use core::ptr;
use std::panic;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{LazyLock, Mutex, PoisonError};

use replicant_game::fight_sync::{
    CAMERA_FOLLOW_UP_LOG_MIN_MS, CAMERA_STATE_LOG_EVERY_MS, FreeDirectionState, RefusedCallStep,
    RefusedCameraCall, RefusedCameraCalls, ScriptedCameraNative, body_and_offset_text,
    camera_call_line, camera_outcome_text, camera_state_line, eye_set_text, fixed_camera_text,
    forward_of_world_matrix, free_direction_matches_request, integer_argument_slots,
    judge_camera_call, read_camera_switch_gates, read_fixed_camera_reading,
    read_free_direction_state, read_native_numbers, requested_blend_seconds,
};
use replicant_game::marker::read_camera_state;
use replicant_game::stay_together::FollowRole;

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::fight_sync::camera_follow::{
    applying_hosts_aim, camera_follows_a_body, camera_native_running, note_change_camera_native,
    own_body,
};
use crate::fight_sync::eye_set_callers::{count_in_window, take_stretch};
use crate::fight_sync::state::{BUILD, current_phase_index, here};
use crate::game_hooks::{hook_address, window_unfocused};
use crate::memory::InProcessMemory;
use crate::net::net_clock_ms;
use crate::stay_together::pause_menu_open;
use crate::story_jump::{NativeArg, call_native_with};

const CAMERA_NATIVE_COUNT: usize = 5;

static ORIGINALS: [AtomicPtr<c_void>; CAMERA_NATIVE_COUNT] =
    [const { AtomicPtr::new(ptr::null_mut()) }; CAMERA_NATIVE_COUNT];
static CAMERA_LOG: LazyLock<Mutex<CameraLog>> = LazyLock::new(Mutex::default);

#[derive(Default)]
struct CameraLog {
    last_state_line_ms: Option<u64>,
    follow_ups: Vec<(String, u64)>,
    refused: RefusedCameraCalls,
}

struct CallBefore {
    line: String,
    free: Option<FreeDirectionState>,
    numbers: Vec<Option<f64>>,
    phase: Option<u16>,
    in_window: bool,
}

type CameraNative = unsafe extern "system" fn(*mut c_void);

pub(super) fn install(build: VerifiedBuild) {
    let detours: [*mut c_void; CAMERA_NATIVE_COUNT] = [
        set_fixed_camera_detour as *const () as *mut c_void,
        change_camera_detour as *const () as *mut c_void,
        direction_fixed_detour as *const () as *mut c_void,
        distance_offset_detour as *const () as *mut c_void,
        direction_with_actor_group_detour as *const () as *mut c_void,
    ];
    for ((native, detour), original) in ScriptedCameraNative::ALL
        .into_iter()
        .zip(detours)
        .zip(&ORIGINALS)
    {
        #[expect(unsafe_code, reason = "hooks a camera script native by its address")]
        // SAFETY: each RVA starts the named native's body in the verified build (docs
        // symbols.tsv): one pointer in (its argument block), nothing out. The detours match it.
        unsafe {
            hook_address(
                native.name(),
                (build.module_base() + u64::from(native.rva())) as usize,
                detour,
                original,
            );
        }
    }
}

pub(super) fn tick(build: &VerifiedBuild) {
    let now_ms = net_clock_ms();
    let phase = current_phase_index(build);
    let in_window = phase.is_some_and(crate::fight_sync::covers_phase);
    let is_guest = here(build).is_some_and(|here| here.role == FollowRole::Guest);
    count_in_window(in_window);
    let mut log = CAMERA_LOG.lock().unwrap_or_else(PoisonError::into_inner);
    let (due, waiting): (Vec<_>, Vec<_>) = log
        .follow_ups
        .drain(..)
        .partition(|(_, due_ms)| now_ms >= *due_ms);
    log.follow_ups = waiting;
    let state_line_due = in_window
        && log
            .last_state_line_ms
            .is_none_or(|at| now_ms.saturating_sub(at) >= CAMERA_STATE_LOG_EVERY_MS);
    if state_line_due {
        log.last_state_line_ms = Some(now_ms);
    }
    let retry = if is_guest {
        log.refused.poll(now_ms, phase)
    } else {
        None
    };
    drop(log);
    for (what, _) in due {
        log_line(&state_line(build, &what));
    }
    if state_line_due {
        let what = format!(
            "each second in the fight (pause menu open {}, window without focus {})",
            pause_menu_open(build),
            window_unfocused()
        );
        let reading = read_fixed_camera_reading(&InProcessMemory::new(build));
        let fixed = fixed_camera_text(reading);
        let body = body_and_offset_text(
            own_body(build),
            reading.map(|reading| reading.pose()),
            camera_follows_a_body(is_guest),
        );
        let eye_sets = eye_set_text(&take_stretch());
        log_line(&format!(
            "{}; {fixed}; {body}; {eye_sets}",
            state_line(build, &what)
        ));
    }
    match retry {
        Some(RefusedCallStep::RunAgain(call)) => run_refused_call_again(build, &call),
        Some(RefusedCallStep::GaveUp(call)) => log_line(&format!(
            "scripted camera: gave up running your refused {} again after {} tries",
            call.native.name(),
            call.tries
        )),
        None => {}
    }
}

fn run_refused_call_again(build: &VerifiedBuild, call: &RefusedCameraCall) {
    let integers = integer_argument_slots(call.native);
    let arguments: Option<Vec<NativeArg>> = call
        .numbers
        .iter()
        .enumerate()
        .map(|(index, number)| {
            number.map(|value| {
                if integers.contains(&index) {
                    NativeArg::Integer(value as i32)
                } else {
                    NativeArg::Real(value)
                }
            })
        })
        .collect();
    let Some(arguments) = arguments else {
        log_line(&format!(
            "scripted camera: your refused {} has an argument that is no number, not run again",
            call.native.name()
        ));
        return;
    };
    log_line(&format!(
        "scripted camera: your game refused its own {}, running it again with its own arguments (try {})",
        call.native.name(),
        call.tries
    ));
    call_native_with(build, call.native.rva(), &arguments);
}

fn state_line(build: &VerifiedBuild, what: &str) -> String {
    let memory = InProcessMemory::new(build);
    let (forward, active_camera) = read_view(build);
    camera_state_line(
        what,
        active_camera,
        forward,
        read_free_direction_state(&memory),
        read_camera_switch_gates(&memory),
    )
}

fn read_view(build: &VerifiedBuild) -> (Option<[f32; 3]>, Option<i32>) {
    match read_camera_state(&InProcessMemory::new(build)) {
        Ok(Some(camera)) => (
            forward_of_world_matrix(&camera.world_matrix),
            Some(camera.active_index),
        ),
        _ => (None, None),
    }
}

fn camera_native_ran(native: ScriptedCameraNative, args: *mut c_void) {
    let trampoline = ORIGINALS[usize::from(native.code() - 1)].load(Ordering::Acquire);
    if applying_hosts_aim() {
        // The mod's own pose update, up to ten a second: not logged, judged or retried.
        if !trampoline.is_null() {
            call_original(trampoline, args);
        }
        return;
    }
    let args_address = args as u64;
    let before = panic::catch_unwind(|| note_call_before(native, args_address))
        .ok()
        .flatten();
    if !trampoline.is_null() {
        camera_native_running(true);
        call_original(trampoline, args);
        camera_native_running(false);
    }
    if native == ScriptedCameraNative::ChangeCamera
        && let Some(before) = &before
    {
        note_change_camera_native(before.in_window);
    }
    if let Some(before) = before {
        let _ = panic::catch_unwind(|| note_call_after(native, &before));
    }
}

fn note_call_after(native: ScriptedCameraNative, before: &CallBefore) {
    log_line(&before.line);
    let Some(build) = BUILD.get() else {
        return;
    };
    let memory = InProcessMemory::new(build);
    let free = read_free_direction_state(&memory);
    let (forward, active_camera) = read_view(build);
    let outcome = judge_camera_call(native, &before.numbers, before.free, active_camera);
    let holds = if native == ScriptedCameraNative::SetFreeCameraDirectionFixed {
        match free_direction_matches_request(&before.numbers, free) {
            Some(true) => ", the free direction holds the request",
            Some(false) => ", the free direction does NOT hold the request",
            None => "",
        }
    } else {
        ""
    };
    let what = format!(
        "right after {}, {}{holds}",
        native.name(),
        camera_outcome_text(outcome)
    );
    log_line(&camera_state_line(
        &what,
        active_camera,
        forward,
        free,
        read_camera_switch_gates(&memory),
    ));
    if !before.in_window {
        return;
    }
    let now_ms = net_clock_ms();
    let blend_ms = (requested_blend_seconds(native, &before.numbers).max(0.0) * 1000.0) as u64;
    let follow_up_ms =
        (blend_ms + CAMERA_FOLLOW_UP_LOG_MIN_MS / 2).max(CAMERA_FOLLOW_UP_LOG_MIN_MS);
    let mut log = CAMERA_LOG.lock().unwrap_or_else(PoisonError::into_inner);
    log.follow_ups.push((
        format!("{follow_up_ms} ms after {}", native.name()),
        now_ms + follow_up_ms,
    ));
    let is_guest = here(build).is_some_and(|here| here.role == FollowRole::Guest);
    if is_guest && let Some(phase) = before.phase {
        log.refused
            .note_outcome(native, &before.numbers, phase, outcome, now_ms);
    }
}

fn note_call_before(native: ScriptedCameraNative, args_address: u64) -> Option<CallBefore> {
    let build = BUILD.get()?;
    let memory = InProcessMemory::new(build);
    let numbers = read_native_numbers(&memory, args_address, native.argument_count());
    let (forward, active_camera) = read_view(build);
    let phase = current_phase_index(build);
    let in_window = phase.is_some_and(crate::fight_sync::covers_phase);
    let line = camera_call_line(native, &numbers, forward, active_camera, in_window);
    Some(CallBefore {
        line,
        free: read_free_direction_state(&memory),
        numbers,
        phase,
        in_window,
    })
}

#[expect(
    unsafe_code,
    reason = "calls the original native through the trampoline"
)]
fn call_original(trampoline: *mut c_void, args: *mut c_void) {
    // SAFETY: the trampoline continues into the original native, which has this signature.
    unsafe { core::mem::transmute::<*mut c_void, CameraNative>(trampoline)(args) };
}

extern "system" fn set_fixed_camera_detour(args: *mut c_void) {
    camera_native_ran(ScriptedCameraNative::SetFixedCamera, args);
}

extern "system" fn change_camera_detour(args: *mut c_void) {
    camera_native_ran(ScriptedCameraNative::ChangeCamera, args);
}

extern "system" fn direction_fixed_detour(args: *mut c_void) {
    camera_native_ran(ScriptedCameraNative::SetFreeCameraDirectionFixed, args);
}

extern "system" fn distance_offset_detour(args: *mut c_void) {
    camera_native_ran(ScriptedCameraNative::SetFreeCameraDistanceOffset, args);
}

extern "system" fn direction_with_actor_group_detour(args: *mut c_void) {
    camera_native_ran(
        ScriptedCameraNative::ResetFreeCameraDirectionWithActorGroup,
        args,
    );
}
