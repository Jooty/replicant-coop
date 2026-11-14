use core::ffi::c_void;
use std::sync::atomic::Ordering;

use replicant_game::downed::{MAP_MANAGER_RVA, RESTART_PHASE_RVA};

use crate::build_gate::VerifiedBuild;
use crate::rewind::state::{ORIGINAL_BACKUP, ORIGINAL_RESTORE};

type SaveBlockCopy = unsafe extern "system" fn();
type RestartPhase = unsafe extern "system" fn(*mut c_void, u32);

pub(crate) fn take_backup(_build: &VerifiedBuild) -> bool {
    let original = ORIGINAL_BACKUP.load(Ordering::Acquire);
    if original.is_null() {
        return false;
    }
    #[expect(
        unsafe_code,
        reason = "calls SaveBlock::BackupToShadow through the trampoline MinHook built"
    )]
    // SAFETY: the trampoline continues into the original BackupToShadow (no arguments) in the
    // verified build; this runs on the game thread inside the script manager tick.
    unsafe {
        core::mem::transmute::<*mut c_void, SaveBlockCopy>(original)();
    }
    true
}

pub(crate) fn rewind_now(build: &VerifiedBuild) -> bool {
    let original = ORIGINAL_RESTORE.load(Ordering::Acquire);
    if original.is_null() {
        return false;
    }
    let map_manager = (build.module_base() + u64::from(MAP_MANAGER_RVA)) as *mut c_void;
    let restart = (build.module_base() + u64::from(RESTART_PHASE_RVA)) as usize;
    crate::teleport::note_mod_move();
    #[expect(
        unsafe_code,
        reason = "calls the game's save block restore and phase restart"
    )]
    // SAFETY: the trampoline continues into the original RestoreFromShadow (no arguments); the
    // restart address is MapManager::RestartPhase (map manager, quick flag 0), called with the
    // global map manager exactly as the _RestartPhase native does; both run on the game thread.
    unsafe {
        core::mem::transmute::<*mut c_void, SaveBlockCopy>(original)();
        core::mem::transmute::<usize, RestartPhase>(restart)(map_manager, 0);
    }
    true
}
