use core::ffi::c_void;
use core::ptr;
use std::panic;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

use replicant_game::GameMemory;
use replicant_game::bombs::{
    BREAKABLE_HITS_LEFT_FOR_ONE_LAST_HIT, BREAKABLE_HITS_LEFT_OFFSET,
    BREAKABLE_RECEIVE_HIT_FUNCTION_RVA, BreakableState, SYNTHETIC_ATTACK_RECORD_BYTES,
    bomb_break_record, break_is_shared, break_started, breakable_class,
};
use replicant_game::effects::read_actor_position;
use replicant_game::offsets::ATTACK_RECORD_FLAGS_OFFSET;
use replicant_proto::{Message, WallBroken};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::door_sync::layout_id_of;
use crate::game_hooks::hook_address;
use crate::memory::InProcessMemory;
use crate::net;

static HOOKED_BUILD: OnceLock<VerifiedBuild> = OnceLock::new();
static ORIGINAL_RECEIVE_HIT: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static COPYING_PARTNER_BREAK: AtomicBool = AtomicBool::new(false);

type ReceiveHit = unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void, i32) -> u64;

#[repr(align(16))]
struct AlignedRecord([u8; SYNTHETIC_ATTACK_RECORD_BYTES]);

pub(super) fn install(build: VerifiedBuild) {
    if HOOKED_BUILD.set(build).is_err() {
        return;
    }
    #[expect(
        unsafe_code,
        reason = "hooks the breakable box classes' shared hit method by address"
    )]
    // SAFETY: the RVA starts vslot 0x558 of CActorBreakableBox and CActorBreakableBox2 (actor,
    // attack record, result flags, part) in the verified build; the replacement has the same
    // four arguments and passes them on unchanged.
    unsafe {
        hook_address(
            "CActorBreakableBox::ReceiveHit",
            (build.module_base() + u64::from(BREAKABLE_RECEIVE_HIT_FUNCTION_RVA)) as usize,
            receive_hit_detour as *const () as *mut c_void,
            &ORIGINAL_RECEIVE_HIT,
        );
    }
}

/// Breaks this game's copy of a wall through the wall's own hit method with a bomb blast's
/// record, so its effect, sound and removal run as for a real blast. True once it is breaking.
pub(super) fn break_wall_here(build: &VerifiedBuild, actor: u64) -> bool {
    let mut memory = InProcessMemory::new(build);
    if memory
        .write_i32(
            actor + BREAKABLE_HITS_LEFT_OFFSET,
            BREAKABLE_HITS_LEFT_FOR_ONE_LAST_HIT,
        )
        .is_err()
    {
        return false;
    }
    let mut record = AlignedRecord(bomb_break_record());
    let mut result = 0_u32;
    let function = (build.module_base() + u64::from(BREAKABLE_RECEIVE_HIT_FUNCTION_RVA)) as usize;
    COPYING_PARTNER_BREAK.store(true, Ordering::Release);
    #[expect(
        unsafe_code,
        reason = "calls the breakable's own hit method with a made-up blast record"
    )]
    // SAFETY: the RVA starts the breakable hit method (actor, record, result, part); the actor was
    // just checked to be a breakable, the record is zeroed past the fields the method reads (the
    // flags at +0xA0 and the given direction at +0x90), both outlive the call, and this runs on
    // the game thread.
    unsafe {
        core::mem::transmute::<usize, ReceiveHit>(function)(
            actor as *mut c_void,
            record.0.as_mut_ptr().cast(),
            (&raw mut result).cast(),
            0,
        );
    }
    COPYING_PARTNER_BREAK.store(false, Ordering::Release);
    BreakableState::read(&memory, actor).is_ok_and(|state| !state.is_whole())
}

#[expect(
    unsafe_code,
    reason = "calls the original hit method through the trampoline MinHook built"
)]
extern "system" fn receive_hit_detour(
    actor: *mut c_void,
    record: *mut c_void,
    result: *mut c_void,
    part: i32,
) -> u64 {
    let before = panic::catch_unwind(|| state_of(actor as u64))
        .ok()
        .flatten();
    let original = ORIGINAL_RECEIVE_HIT.load(Ordering::Acquire);
    if original.is_null() {
        return 0;
    }
    // SAFETY: the trampoline continues into the original method, which has this exact signature.
    let outcome = unsafe {
        core::mem::transmute::<*mut c_void, ReceiveHit>(original)(actor, record, result, part)
    };
    if let Some(before) = before {
        let _ = panic::catch_unwind(|| after_hit(actor as u64, record as u64, before));
    }
    outcome
}

fn state_of(actor: u64) -> Option<BreakableState> {
    let build = HOOKED_BUILD.get()?;
    BreakableState::read(&InProcessMemory::new(build), actor).ok()
}

fn after_hit(actor: u64, record: u64, before: BreakableState) {
    let Some(build) = HOOKED_BUILD.get() else {
        return;
    };
    let memory = InProcessMemory::new(build);
    let Ok(after) = BreakableState::read(&memory, actor) else {
        return;
    };
    if !break_started(before, after) {
        return;
    }
    let position = read_actor_position(&memory, actor).unwrap_or([0.0; 3]);
    let class = breakable_class(&memory, actor);
    if COPYING_PARTNER_BREAK.load(Ordering::Acquire) {
        log_line(&format!(
            "walls: the partner's break of {class:?} at ({:.1}, {:.1}, {:.1}) is copied here",
            position[0], position[1], position[2]
        ));
        return;
    }
    let flags = memory
        .read_i32(record + ATTACK_RECORD_FLAGS_OFFSET)
        .map_or(0, i32::cast_unsigned);
    let shared = break_is_shared(before, flags);
    let layout_id = u16::try_from(layout_id_of(build, actor as *mut c_void)).ok();
    log_line(&format!(
        "walls: {class:?} layout {layout_id:?} at ({:.1}, {:.1}, {:.1}) broke here (mode {}, hits left {}, hit flags {flags:#x}), shared {shared}",
        position[0], position[1], position[2], before.mode, before.hits_left
    ));
    if !shared {
        return;
    }
    let Some(net) = net::handle() else {
        return;
    };
    let view = net.view();
    let Some(phase) = view.local.as_ref().map(|local| local.phase) else {
        return;
    };
    let wall = WallBroken {
        phase,
        layout_id,
        position,
    };
    if wall.validate().is_ok() {
        net.send_relayed(Message::WallBroken(wall));
        log_line(&format!("walls: sent the break for zone {phase}"));
    }
}
