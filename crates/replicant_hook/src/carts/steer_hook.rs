use core::ffi::c_void;
use core::ptr;
use std::panic;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError, TryLockError};

use replicant_game::carts::{
    CART_RUN_MOTION_LAYER, CART_SET_MOTION_RATE_FUNCTION_RVA, CART_UPDATE_FUNCTION_RVA, CartMotion,
    CartState, CartSteer, CartSteerSample, HostCartRun, NEWER_MOTION_LAYER_SET_FRAME_FUNCTION_RVA,
    OLD_MOTION_LAYER_SET_FRAME_FUNCTION_RVA, read_cart_motion, read_cart_run_end_frame,
    read_cart_state, steer_cart,
};

use crate::build_gate::VerifiedBuild;
use crate::game_hooks::hook_address;
use crate::memory::InProcessMemory;
use crate::net::net_clock_ms;

static ORIGINAL_CART_UPDATE: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static STEERED_CART: AtomicU64 = AtomicU64::new(0);
static STEERING_NOW: AtomicBool = AtomicBool::new(false);
static STEER_BUILD: OnceLock<VerifiedBuild> = OnceLock::new();
static STEERING: Mutex<Steering> = Mutex::new(Steering::new());

type CartUpdate = unsafe extern "system" fn(*mut c_void, u64, u64, u64) -> u64;
type SetMotionRate = unsafe extern "system" fn(*mut c_void, i32, f32);
type SetLayerFrame = unsafe extern "system" fn(*mut c_void, f32);

/// A frame the steering set on the guest's cart, kept until the cart tick logs it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct FrameSet {
    pub from: f32,
    pub to: f32,
    pub gap: f32,
}

/// What the cart tick hands the guest's cart update, and what the update hands back for the log.
struct Steering {
    actor: u64,
    host: Option<HostCartRun>,
    last: Option<CartSteerSample>,
    frame_set: Option<FrameSet>,
}

impl Steering {
    const fn new() -> Self {
        Self {
            actor: 0,
            host: None,
            last: None,
            frame_set: None,
        }
    }
}

pub(super) fn install_steering(build: VerifiedBuild) {
    let _ = STEER_BUILD.set(build);
    #[expect(
        unsafe_code,
        reason = "hooks the cart actor's update method by address"
    )]
    // SAFETY: the RVA starts CActorTruck's update method (vtable slot 0x4E0) in the verified
    // build; the replacement takes the same arguments and passes them on unchanged.
    unsafe {
        hook_address(
            "CActorTruck_Update",
            (build.module_base() + u64::from(CART_UPDATE_FUNCTION_RVA)) as usize,
            cart_update_detour as *const () as *mut c_void,
            &ORIGINAL_CART_UPDATE,
        );
    }
}

pub(super) fn steer_guest_cart(
    actor: Option<u64>,
    host: Option<HostCartRun>,
) -> (Option<CartSteerSample>, Option<FrameSet>) {
    let mut steering = STEERING.lock().unwrap_or_else(PoisonError::into_inner);
    let actor = actor.unwrap_or(0);
    if steering.actor != actor {
        steering.last = None;
    }
    steering.actor = actor;
    steering.host = host.filter(|_| actor != 0);
    STEERED_CART.store(actor, Ordering::Release);
    let last = if actor == 0 { None } else { steering.last };
    (last, steering.frame_set.take())
}

#[expect(
    unsafe_code,
    reason = "calls the original cart update method through the trampoline MinHook built"
)]
extern "system" fn cart_update_detour(
    cart: *mut c_void,
    second: u64,
    third: u64,
    fourth: u64,
) -> u64 {
    let original = ORIGINAL_CART_UPDATE.load(Ordering::Acquire);
    if original.is_null() {
        return 0;
    }
    // SAFETY: the trampoline continues into the original method, which has this exact signature.
    let result = unsafe {
        core::mem::transmute::<*mut c_void, CartUpdate>(original)(cart, second, third, fourth)
    };
    let actor = cart as u64;
    // Every other cart, and this one outside a ride the host started, costs one load.
    if actor != 0
        && STEERED_CART.load(Ordering::Acquire) == actor
        && !STEERING_NOW.swap(true, Ordering::AcqRel)
    {
        let _ = panic::catch_unwind(|| steer_after_update(actor));
        STEERING_NOW.store(false, Ordering::Release);
    }
    result
}

fn steer_after_update(actor: u64) {
    let Some(build) = STEER_BUILD.get() else {
        return;
    };
    let mut steering = match STEERING.try_lock() {
        Ok(steering) => steering,
        Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
        Err(TryLockError::WouldBlock) => return,
    };
    if steering.actor != actor {
        return;
    }
    let memory = InProcessMemory::new(build);
    if !matches!(read_cart_state(&memory, actor), Some(CartState::Running(_))) {
        return;
    }
    let Some(motion) = read_cart_motion(&memory, actor) else {
        return;
    };
    let end_frame = read_cart_run_end_frame(&memory, actor);
    let steer = steer_cart(
        steering.host.as_ref(),
        motion.frame,
        end_frame,
        net_clock_ms(),
    );
    match steer {
        CartSteer::LeaveAlone(_) => {}
        CartSteer::Rate { rate, .. } => set_run_rate(build, actor, rate),
        CartSteer::SetFrame { frame, rate, gap } => {
            set_run_frame(build, &motion, frame);
            set_run_rate(build, actor, rate);
            steering.frame_set = Some(FrameSet {
                from: motion.frame,
                to: frame,
                gap,
            });
        }
    }
    steering.last = Some(CartSteerSample {
        own_frame: motion.frame,
        newer_motion: motion.newer,
        steer,
    });
}

#[expect(
    unsafe_code,
    reason = "calls the game's motion rate setter by its address"
)]
fn set_run_rate(build: &VerifiedBuild, actor: u64, rate: f32) {
    let function =
        (build.module_base() + u64::from(CART_SET_MOTION_RATE_FUNCTION_RVA)) as *const ();
    // SAFETY: the RVA starts CActor::SetMotionRate (rcx actor, edx layer, xmm2 rate) in the
    // verified build, which picks the actor's motion code itself and skips a missing layer; the
    // actor is the cart whose update just ran, on the game thread.
    unsafe {
        core::mem::transmute::<*const (), SetMotionRate>(function)(
            actor as *mut c_void,
            CART_RUN_MOTION_LAYER,
            rate,
        );
    }
}

#[expect(
    unsafe_code,
    reason = "calls the game's motion layer frame setter by its address"
)]
fn set_run_frame(build: &VerifiedBuild, motion: &CartMotion, frame: f32) {
    let rva = if motion.newer {
        NEWER_MOTION_LAYER_SET_FRAME_FUNCTION_RVA
    } else {
        OLD_MOTION_LAYER_SET_FRAME_FUNCTION_RVA
    };
    let function = (build.module_base() + u64::from(rva)) as *const ();
    // SAFETY: the RVA starts the frame setter of the motion code `motion.newer` names (rcx layer,
    // xmm1 frame) in the verified build; the layer was just read from the cart whose update ran,
    // and the setter itself skips an empty slot. This runs on the game thread.
    unsafe {
        core::mem::transmute::<*const (), SetLayerFrame>(function)(
            motion.layer as *mut c_void,
            frame,
        );
    }
}
