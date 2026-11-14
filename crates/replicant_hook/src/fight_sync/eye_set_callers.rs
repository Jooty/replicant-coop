#![expect(
    unsafe_code,
    reason = "the naked SetEye stub, which no item attribute can carry"
)]

use core::ffi::c_void;
use core::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use replicant_game::fight_sync::{EyeSetStretch, EyeSetTally, FIXED_CAMERA_SET_EYE_RVA};

use crate::build_gate::VerifiedBuild;
use crate::fight_sync::camera_follow::mod_is_setting_the_camera;
use crate::game_hooks::hook_address;

static SET_EYE_ORIGINAL: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static IMAGE_BASE: AtomicU64 = AtomicU64::new(0);
static COUNTING: AtomicBool = AtomicBool::new(false);
static RECORDING_NOW: AtomicBool = AtomicBool::new(false);
static TALLY: Mutex<EyeSetTally> = Mutex::new(EyeSetTally::new());

type SetEye = unsafe extern "system" fn(*mut c_void, *const c_void, f32);

pub(super) fn install(build: VerifiedBuild) {
    IMAGE_BASE.store(build.module_base(), Ordering::Release);
    #[expect(unsafe_code, reason = "hooks FixedCamera::SetEye by its address")]
    // SAFETY: the RVA starts `FixedCamera::SetEye` in the verified build (symbols.tsv): camera,
    // eye pointer, lerp seconds (xmm2). The stub only adds the return address in r9 and jumps on.
    unsafe {
        hook_address(
            "FixedCamera::SetEye",
            (build.module_base() + u64::from(FIXED_CAMERA_SET_EYE_RVA)) as usize,
            set_eye_stub as *const () as *mut c_void,
            &SET_EYE_ORIGINAL,
        );
    }
}

pub(super) fn count_in_window(in_window: bool) {
    COUNTING.store(in_window, Ordering::Release);
}

pub(super) fn take_stretch() -> EyeSetStretch {
    TALLY.lock().unwrap_or_else(PoisonError::into_inner).take()
}

#[unsafe(naked)]
extern "system" fn set_eye_stub() {
    // On entry [rsp] is the game caller's return address; r9 (the fourth argument) is unused by
    // SetEye. A jump, not a call, so the detour returns straight to the game.
    core::arch::naked_asm!(
        "mov r9, qword ptr [rsp]",
        "jmp {detour}",
        detour = sym set_eye_detour,
    );
}

#[expect(
    unsafe_code,
    reason = "calls the original SetEye through the trampoline MinHook built"
)]
extern "system" fn set_eye_detour(
    camera: *mut c_void,
    eye: *const c_void,
    seconds: f32,
    caller: u64,
) {
    let original = SET_EYE_ORIGINAL.load(Ordering::Acquire);
    if !original.is_null() {
        // SAFETY: the trampoline continues into the original function, which has this signature.
        unsafe { core::mem::transmute::<*mut c_void, SetEye>(original)(camera, eye, seconds) };
    }
    if !COUNTING.load(Ordering::Acquire) || RECORDING_NOW.swap(true, Ordering::AcqRel) {
        return;
    }
    let offset =
        u32::try_from(caller.wrapping_sub(IMAGE_BASE.load(Ordering::Acquire))).unwrap_or(u32::MAX);
    if let Ok(mut tally) = TALLY.try_lock() {
        tally.record(offset, mod_is_setting_the_camera());
    }
    RECORDING_NOW.store(false, Ordering::Release);
}
