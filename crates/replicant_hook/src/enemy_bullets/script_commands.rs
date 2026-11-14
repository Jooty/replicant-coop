use core::cell::Cell;
use core::ffi::c_void;
use core::ptr;
use std::collections::HashSet;
use std::panic;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock, PoisonError};

use replicant_game::enemies::ActorHandle;
use replicant_game::enemy_bullets::{
    SCRIPT_BULLET_COMMAND_RECEIVER_OFFSET, SCRIPT_BULLET_EXECUTE_COMMAND_RVA,
};
use replicant_game::offsets::NORMAL_BULLET_OWNER_HANDLE_OFFSET;

use crate::build_gate::VerifiedBuild;
use crate::enemy_bullets::shot_cues::{self, begin_command, end_command, restore_outer_command};
use crate::game_hooks::hook_address;
use crate::memory::InProcessMemory;

static ORIGINAL_EXECUTE_COMMAND: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static HOOKED_BUILD: OnceLock<VerifiedBuild> = OnceLock::new();
static SHARED_ENEMY_ACTORS: LazyLock<Mutex<HashSet<u64>>> = LazyLock::new(Mutex::default);

thread_local! {
    static COMMAND_IN_FLIGHT: Cell<Option<(u64, i32)>> = const { Cell::new(None) };
}

type ExecuteCommand = unsafe extern "system" fn(u64, i32, u64, u32, u64, u64) -> u64;

pub fn install(build: VerifiedBuild) {
    if HOOKED_BUILD.set(build).is_err() {
        return;
    }
    shot_cues::install(build);
    let address = (build.module_base() + u64::from(SCRIPT_BULLET_EXECUTE_COMMAND_RVA)) as usize;
    #[expect(
        unsafe_code,
        reason = "hooks the script bullet's command interpreter by its address"
    )]
    // SAFETY: the RVA starts `CScriptBullet::ExecuteScriptCommand` in the verified build (docs
    // symbols.tsv): six integer or pointer arguments, the first the bullet + 0x1900, and an
    // integer result. The replacement has the same signature and forwards every argument.
    unsafe {
        hook_address(
            "CScriptBullet::ExecuteScriptCommand",
            address,
            execute_command_detour as *const () as *mut c_void,
            &ORIGINAL_EXECUTE_COMMAND,
        );
    }
}

/// The host's shared enemies this scan; an empty set while not hosting a partner in play.
pub(crate) fn set_shared_enemy_actors(actors: impl Iterator<Item = u64>) {
    let mut shared = SHARED_ENEMY_ACTORS
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    shared.clear();
    shared.extend(actors);
}

/// The bullet and the command of the innermost bullet script command running on this thread.
pub(crate) fn command_in_flight() -> Option<(u64, i32)> {
    COMMAND_IN_FLIGHT.with(Cell::get)
}

/// `true` when a shared enemy of this host fired `bullet`.
pub(crate) fn bullet_is_shared(bullet: u64) -> bool {
    let Some(build) = HOOKED_BUILD.get() else {
        return false;
    };
    let shared = SHARED_ENEMY_ACTORS
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if shared.is_empty() {
        return false;
    }
    let memory = InProcessMemory::new(build);
    ActorHandle::read(&memory, bullet + NORMAL_BULLET_OWNER_HANDLE_OFFSET)
        .and_then(|handle| handle.resolve(&memory))
        .is_some_and(|owner| shared.contains(&owner))
}

extern "system" fn execute_command_detour(
    receiver: u64,
    command: i32,
    arguments: u64,
    fourth: u32,
    fifth: u64,
    sixth: u64,
) -> u64 {
    let trampoline = ORIGINAL_EXECUTE_COMMAND.load(Ordering::Acquire);
    if trampoline.is_null() {
        return 0;
    }
    let bullet = receiver.wrapping_sub(SCRIPT_BULLET_COMMAND_RECEIVER_OFFSET);
    let outer = COMMAND_IN_FLIGHT.with(|cell| cell.replace(Some((bullet, command))));
    let outer_made = begin_command();
    let result = call_original(
        trampoline, receiver, command, arguments, fourth, fifth, sixth,
    );
    let _ = panic::catch_unwind(|| end_command(bullet, command));
    restore_outer_command(outer_made);
    COMMAND_IN_FLIGHT.with(|cell| cell.set(outer));
    result
}

#[expect(
    unsafe_code,
    reason = "calls the original interpreter through the trampoline"
)]
fn call_original(
    trampoline: *mut c_void,
    receiver: u64,
    command: i32,
    arguments: u64,
    fourth: u32,
    fifth: u64,
    sixth: u64,
) -> u64 {
    // SAFETY: the trampoline continues into the original interpreter, which has this signature.
    unsafe {
        core::mem::transmute::<*mut c_void, ExecuteCommand>(trampoline)(
            receiver, command, arguments, fourth, fifth, sixth,
        )
    }
}
