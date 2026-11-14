use core::ffi::c_void;
use core::ptr;
use std::panic;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};

use replicant_game::Replicant;
use replicant_game::offsets::{
    ACTOR_COLLISION_RESOLVER_FUNCTION_RVA, ACTOR_MOVE_STEP_FUNCTION_RVA,
};

use crate::build_gate::VerifiedBuild;
use crate::game_hooks::noclip_keys::read_noclip_input;
use crate::game_hooks::teleport;
use crate::memory::InProcessMemory;
use nc_hook::detour::hook_address;

static ENABLED: AtomicBool = AtomicBool::new(false);
static ORIGINAL_MOVE_STEP: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static ORIGINAL_RESOLVER: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static ACTOR_SKIPPING_RESOLVER: AtomicUsize = AtomicUsize::new(0);
static HOOKED_BUILD: OnceLock<VerifiedBuild> = OnceLock::new();

type MoveStep = unsafe extern "system" fn(usize, usize, usize, usize, usize, usize) -> usize;
type CollisionResolver = unsafe extern "system" fn(usize) -> usize;

pub(super) fn install(build: VerifiedBuild) {
    if HOOKED_BUILD.set(build).is_err() {
        return;
    }
    let base = build.module_base();
    let move_step_address = (base + u64::from(ACTOR_MOVE_STEP_FUNCTION_RVA)) as usize;
    let resolver_address = (base + u64::from(ACTOR_COLLISION_RESOLVER_FUNCTION_RVA)) as usize;
    let move_step_replacement = move_step_detour as *const () as *mut c_void;
    let resolver_replacement = collision_resolver_detour as *const () as *mut c_void;
    #[expect(unsafe_code, reason = "hooks the actor move step by its address")]
    // SAFETY: the RVA starts the per-actor move step (rcx world, rdx, r8d, r9 actor, one stack
    // argument) in the verified build; the replacement forwards six integer slots, which covers it.
    let hooked_move_step = unsafe {
        hook_address(
            "actor move step",
            move_step_address,
            move_step_replacement,
            &ORIGINAL_MOVE_STEP,
        )
    };
    if !hooked_move_step {
        return;
    }
    #[expect(unsafe_code, reason = "hooks the collision resolver by its address")]
    // SAFETY: the RVA starts the ActorPlayable collision resolver (rcx actor, returns bool) in the
    // verified build, and the replacement has the same shape.
    unsafe {
        hook_address(
            "collision resolver",
            resolver_address,
            resolver_replacement,
            &ORIGINAL_RESOLVER,
        )
    };
}

pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn is_installed() -> bool {
    !ORIGINAL_MOVE_STEP.load(Ordering::Acquire).is_null()
        && !ORIGINAL_RESOLVER.load(Ordering::Acquire).is_null()
}

#[expect(
    unsafe_code,
    reason = "calls the original move step through the trampoline MinHook built"
)]
extern "system" fn move_step_detour(
    world: usize,
    second: usize,
    third: usize,
    actor: usize,
    fifth: usize,
    sixth: usize,
) -> usize {
    let original = ORIGINAL_MOVE_STEP.load(Ordering::Acquire);
    if original.is_null() {
        return 0;
    }
    let _ = panic::catch_unwind(|| crate::enemies::cut_copy_step_before_move(actor as u64));
    let driven = (ENABLED.load(Ordering::Relaxed) && drive_player(actor))
        || HOOKED_BUILD
            .get()
            .is_some_and(|build| teleport::apply_pending(build, actor));
    if driven {
        ACTOR_SKIPPING_RESOLVER.store(actor, Ordering::Release);
    }
    // SAFETY: the trampoline continues into the original move step; the extra stack slots read
    // here belong to the caller's frame and are passed back unchanged.
    let result = unsafe {
        core::mem::transmute::<*mut c_void, MoveStep>(original)(
            world, second, third, actor, fifth, sixth,
        )
    };
    if driven {
        ACTOR_SKIPPING_RESOLVER.store(0, Ordering::Release);
    }
    result
}

#[expect(
    unsafe_code,
    reason = "calls the original resolver through the trampoline MinHook built"
)]
extern "system" fn collision_resolver_detour(actor: usize) -> usize {
    if actor != 0 && ACTOR_SKIPPING_RESOLVER.load(Ordering::Acquire) == actor {
        return 0;
    }
    let original = ORIGINAL_RESOLVER.load(Ordering::Acquire);
    if original.is_null() {
        return 0;
    }
    // SAFETY: the trampoline continues into the original resolver, which has this exact signature.
    unsafe { core::mem::transmute::<*mut c_void, CollisionResolver>(original)(actor) }
}

fn drive_player(actor: usize) -> bool {
    let Some(build) = HOOKED_BUILD.get() else {
        return false;
    };
    panic::catch_unwind(|| {
        let mut game = Replicant::new(InProcessMemory::new(build));
        let is_local_player = game
            .playable_actor()
            .is_ok_and(|player| player == actor as u64);
        if is_local_player {
            crate::teleport::note_mod_move();
        }
        is_local_player
            && game
                .write_noclip_step(actor as u64, read_noclip_input().displacement())
                .is_ok()
    })
    .unwrap_or(false)
}
