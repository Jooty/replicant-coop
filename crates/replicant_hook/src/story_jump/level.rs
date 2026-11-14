use core::ffi::c_void;
use core::ptr;
use std::panic;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{Mutex, PoisonError};

use replicant_game::Replicant;
use replicant_game::story_jump::{
    LevelCheck, LevelWatcher, NATIVE_ARG_FIRST_OFFSET_BYTES, NATIVE_ARG_STRIDE_BYTES,
    NATIVE_ARG_VALUE_OFFSET_BYTES, NATIVE_ARGS_HEADER_BYTES, PLAYER_UNIQUE_ACTOR_ID,
    SET_UNIQUE_ACTOR_LEVEL_NATIVE_RVA, check_level, level_check_is_due, loadout_player_level,
};
use replicant_proto::StoryJumpPoint;

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::game_hooks::{hook_address, note_native};
use crate::memory::InProcessMemory;
use crate::net::net_clock_ms;
use crate::story_jump::natives::call_native;
use crate::story_writes::calls::running_call;

const LEVEL_CHECK_GIVE_UP_MS: u64 = 60_000;

static ORIGINAL_SET_UNIQUE_ACTOR_LEVEL: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static WATCHER: Mutex<LevelWatcher> = Mutex::new(LevelWatcher::new());
static EXPECTED: Mutex<Option<ExpectedLevel>> = Mutex::new(None);

type LevelNative = unsafe extern "system" fn(*mut c_void);

struct ExpectedLevel {
    level: i32,
    phase_at_apply: String,
    applied_ms: u64,
    phase_changed_ms: Option<u64>,
}

pub(crate) fn install(build: VerifiedBuild) {
    #[expect(
        unsafe_code,
        reason = "hooks the set-actor-level script native by address to log its calls"
    )]
    // SAFETY: the RVA starts _SetUniqueActorLevel in the verified build (docs design/story-jump.md).
    unsafe {
        hook_address(
            "LuaNative::SetUniqueActorLevel",
            (build.module_base() + u64::from(SET_UNIQUE_ACTOR_LEVEL_NATIVE_RVA)) as usize,
            set_unique_actor_level_detour as *const () as *mut c_void,
            &ORIGINAL_SET_UNIQUE_ACTOR_LEVEL,
        );
    }
}

#[expect(
    unsafe_code,
    reason = "reads the native's argument block and calls the original through the trampoline"
)]
extern "system" fn set_unique_actor_level_detour(args: *mut c_void) {
    let args_address = args as u64;
    let _ = panic::catch_unwind(|| {
        note_native("_SetUniqueActorLevel");
        log_level_call(args_address);
    });
    let trampoline = ORIGINAL_SET_UNIQUE_ACTOR_LEVEL.load(Ordering::Acquire);
    if trampoline.is_null() {
        return;
    }
    // SAFETY: the trampoline continues into the original native, which has this signature.
    unsafe { core::mem::transmute::<*mut c_void, LevelNative>(trampoline)(args) };
}

#[expect(
    unsafe_code,
    reason = "reads two integer arguments from the block the game passes to the native"
)]
fn log_level_call(args_address: u64) {
    let read_argument = |index: usize| -> i32 {
        let offset = NATIVE_ARGS_HEADER_BYTES
            + NATIVE_ARG_FIRST_OFFSET_BYTES
            + index * NATIVE_ARG_STRIDE_BYTES
            + NATIVE_ARG_VALUE_OFFSET_BYTES;
        // SAFETY: the native itself reads both integer arguments from this block, so both slots exist.
        unsafe { ((args_address as usize + offset) as *const i32).read_unaligned() }
    };
    let actor = read_argument(0);
    let level = read_argument(1);
    let caller = running_call().map_or_else(
        || "none".to_owned(),
        |call| format!("{:#x}", call.name_hash),
    );
    let for_player = if actor == PLAYER_UNIQUE_ACTOR_ID {
        "player"
    } else {
        "other actor"
    };
    log_line(&format!(
        "level: _SetUniqueActorLevel({actor}, {level}) for {for_player}, script call {caller}"
    ));
}

pub(super) fn read_player_level(build: &VerifiedBuild) -> Option<i32> {
    Replicant::new(InProcessMemory::new(build)).level().ok()
}

pub(super) fn log_level(build: &VerifiedBuild, when: &str) {
    match read_player_level(build) {
        Some(level) => log_line(&format!("story jump: level {level} {when}")),
        None => log_line(&format!("story jump: level unreadable {when}")),
    }
}

pub(super) fn expect_level_after_jump(build: &VerifiedBuild, point: StoryJumpPoint) {
    let Some(level) = loadout_player_level(point) else {
        return;
    };
    let game = Replicant::new(InProcessMemory::new(build));
    let phase_at_apply = game.current_phase().unwrap_or_default();
    *EXPECTED.lock().unwrap_or_else(PoisonError::into_inner) = Some(ExpectedLevel {
        level,
        phase_at_apply,
        applied_ms: net_clock_ms(),
        phase_changed_ms: None,
    });
}

pub(super) fn tick_level(build: &VerifiedBuild) {
    watch_level(build);
    verify_expected_level(build);
}

fn watch_level(build: &VerifiedBuild) {
    let Some(level) = read_player_level(build) else {
        return;
    };
    let change = WATCHER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .observe(level);
    let Some((before, after)) = change else {
        return;
    };
    let before_text = before.map_or_else(|| "unknown".to_owned(), |value| value.to_string());
    let caller = running_call().map_or_else(
        || "none".to_owned(),
        |call| format!("{:#x}", call.name_hash),
    );
    log_line(&format!(
        "level: {before_text} -> {after} (last callback {caller})"
    ));
}

fn verify_expected_level(build: &VerifiedBuild) {
    let mut guard = EXPECTED.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(expected) = guard.as_mut() else {
        return;
    };
    let now = net_clock_ms();
    if now.saturating_sub(expected.applied_ms) > LEVEL_CHECK_GIVE_UP_MS {
        log_line("story jump: level check gave up, the phase never changed");
        *guard = None;
        return;
    }
    let game = Replicant::new(InProcessMemory::new(build));
    let phase_now = game.current_phase().unwrap_or_default();
    if phase_now == expected.phase_at_apply {
        return;
    }
    let changed_ms = *expected.phase_changed_ms.get_or_insert(now);
    let field_ready = game.map_manager_is_active().unwrap_or(false);
    if !level_check_is_due(
        &expected.phase_at_apply,
        &phase_now,
        field_ready,
        now.saturating_sub(changed_ms),
    ) {
        return;
    }
    let level = expected.level;
    *guard = None;
    drop(guard);
    match check_level(level, read_player_level(build)) {
        LevelCheck::Matches => log_line(&format!(
            "story jump: level {level} held after the travel ({phase_now})"
        )),
        LevelCheck::ApplyAgain { expected, actual } => {
            log_line(&format!(
                "story jump: level is {actual} after the travel, expected {expected}, applying it again"
            ));
            call_native(
                build,
                SET_UNIQUE_ACTOR_LEVEL_NATIVE_RVA,
                &[PLAYER_UNIQUE_ACTOR_ID, expected],
            );
            log_level(build, "after applying it again");
        }
        LevelCheck::Unreadable => log_line("story jump: level unreadable after the travel"),
    }
}
