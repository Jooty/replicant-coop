use core::ffi::c_void;
use core::ptr;
use std::panic;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicPtr, Ordering};

use crate::build_gate::VerifiedBuild;
use crate::detour::Detour;
use crate::diagnostics::log_line;
use crate::stall_watch::note_game_thread_tick;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;

static ORIGINAL_TICK: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static HOOKED_BUILD: OnceLock<VerifiedBuild> = OnceLock::new();
static ON_TICK: OnceLock<fn(&VerifiedBuild)> = OnceLock::new();

type GameThreadTick = unsafe extern "system" fn(*mut c_void);

pub fn install_game_thread_tick(build: VerifiedBuild, tick_rva: u32, on_tick: fn(&VerifiedBuild)) {
    if HOOKED_BUILD.set(build).is_err() {
        return;
    }
    let _ = ON_TICK.set(on_tick);
    let replacement = game_thread_tick_detour as *const () as *mut c_void;
    #[expect(
        unsafe_code,
        reason = "hooks the per-tick function; the replacement has the same signature"
    )]
    // SAFETY: `tick_rva` starts a per-tick function (one pointer argument, no result) in the
    // verified build, and the replacement takes and passes on the same single argument.
    let created = unsafe { Detour::create(&build, tick_rva, replacement) };
    let detour = match created {
        Ok(detour) => detour,
        Err(error) => {
            log_line(&format!("actions: cannot hook the script tick ({error})"));
            return;
        }
    };
    ORIGINAL_TICK.store(detour.trampoline(), Ordering::Release);
    match detour.enable() {
        Ok(()) => log_line("actions: script tick hooked, game-thread queue is live"),
        Err(error) => {
            ORIGINAL_TICK.store(ptr::null_mut(), Ordering::Release);
            log_line(&format!(
                "actions: cannot enable the script tick hook ({error})"
            ));
        }
    }
}

pub fn is_draining() -> bool {
    !ORIGINAL_TICK.load(Ordering::Acquire).is_null()
}

#[expect(
    unsafe_code,
    reason = "calls the original tick through the trampoline MinHook built, and reads its thread id"
)]
extern "system" fn game_thread_tick_detour(context: *mut c_void) {
    // SAFETY: GetCurrentThreadId takes no arguments and cannot fail.
    note_game_thread_tick(unsafe { GetCurrentThreadId() });
    if let (Some(build), Some(on_tick)) = (HOOKED_BUILD.get(), ON_TICK.get())
        && panic::catch_unwind(|| on_tick(build)).is_err()
    {
        log_line("actions: panic while running queued actions");
    }
    let original = ORIGINAL_TICK.load(Ordering::Acquire);
    if original.is_null() {
        return;
    }
    // SAFETY: the trampoline continues into the original tick, which has this exact signature.
    unsafe {
        core::mem::transmute::<*mut c_void, GameThreadTick>(original)(context);
    }
}
