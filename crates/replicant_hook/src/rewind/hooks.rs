use core::ffi::c_void;
use std::panic;
use std::sync::PoisonError;
use std::sync::atomic::{AtomicPtr, Ordering};

use replicant_game::downed::{BACKUP_SAVE_DATA_RVA, RESTORE_SAVE_DATA_RVA};
use replicant_game::stay_together::FollowRole;
use replicant_proto::{CombatEvent, Message};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::event_log::post_event;
use crate::game_hooks::hook_address;
use crate::net::{self, NetState, net_clock_ms};
use crate::rewind::state::{GAME_OVER_GATE, HOOKED_BUILD, ORIGINAL_BACKUP, ORIGINAL_RESTORE};

type SaveBlockCopy = unsafe extern "system" fn();

pub fn install(build: VerifiedBuild) {
    if HOOKED_BUILD.set(build).is_err() {
        return;
    }
    #[expect(
        unsafe_code,
        reason = "hooks the save block backup and restore functions by address"
    )]
    // SAFETY: both RVAs start functions taking no arguments in the verified build (docs
    // off-hand/story-events.md); each replacement takes none and calls the original first.
    unsafe {
        hook_address(
            "SaveBlock::BackupToShadow",
            (build.module_base() + u64::from(BACKUP_SAVE_DATA_RVA)) as usize,
            backup_detour as *const () as *mut c_void,
            &ORIGINAL_BACKUP,
        );
        hook_address(
            "SaveBlock::RestoreFromShadow",
            (build.module_base() + u64::from(RESTORE_SAVE_DATA_RVA)) as usize,
            restore_detour as *const () as *mut c_void,
            &ORIGINAL_RESTORE,
        );
    }
}

extern "system" fn backup_detour() {
    run_original(&ORIGINAL_BACKUP);
    let _ = panic::catch_unwind(host_sends_checkpoint);
}

extern "system" fn restore_detour() {
    run_original(&ORIGINAL_RESTORE);
    let _ = panic::catch_unwind(host_sends_game_over);
}

#[expect(
    unsafe_code,
    reason = "calls an original save block function through the trampoline MinHook built"
)]
fn run_original(original: &AtomicPtr<c_void>) {
    let original = original.load(Ordering::Acquire);
    if original.is_null() {
        return;
    }
    // SAFETY: the trampoline continues into the original function, which takes no arguments.
    unsafe { core::mem::transmute::<*mut c_void, SaveBlockCopy>(original)() };
}

fn host_sends_checkpoint() {
    let Some(net) = net::handle() else {
        return;
    };
    let view = net.view();
    if view.state != NetState::Connected
        || FollowRole::from_slot(view.local_slot) != FollowRole::Host
    {
        return;
    }
    net.send_relayed(Message::Combat(CombatEvent::HostCheckpoint));
    log_line("rewind: host took a checkpoint, sent it to the guest");
}

fn host_sends_game_over() {
    let Some(net) = net::handle() else {
        return;
    };
    let view = net.view();
    if view.state != NetState::Connected
        || FollowRole::from_slot(view.local_slot) != FollowRole::Host
    {
        return;
    }
    let from_game_over = GAME_OVER_GATE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take(net_clock_ms());
    if !from_game_over {
        log_line("rewind: host restored its save block outside the game over path, guest stays");
        return;
    }
    net.send_relayed(Message::Combat(CombatEvent::HostGameOver));
    log_line("rewind: host restored its checkpoint after a game over, sent it to the guest");
    post_event(&format!(
        "{} goes back with you",
        view.partner_name().unwrap_or("The other player")
    ));
}
