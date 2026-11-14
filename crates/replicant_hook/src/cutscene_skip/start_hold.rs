use core::ffi::c_void;
use core::ptr;
use std::panic;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError};

use replicant_game::cutscene_skip::{
    EVENT_CUT_LOADED_BYTE_RVA, SCENE_ITEM_SCENE_ID_OFFSET, SCENE_ITEM_STATE_NOT_STARTED,
    SCENE_ITEM_STATE_OFFSET, SCENE_ITEM_STATE_STARTING, SCENE_ITEM_UPDATE_RVA,
    SCENE_START_HOLD_TIMEOUT_MS, SceneStartGate, StartVerdict,
};
use replicant_game::stay_together::FollowRole;
use replicant_game::{GameMemory, PhaseEntry, Replicant};

use crate::build_gate::VerifiedBuild;
use crate::cutscene_skip::state::partner_in_cutscene;
use crate::diagnostics::log_line;
use crate::game_hooks::hook_address;
use crate::memory::InProcessMemory;
use crate::net::NetSharedExt;
use crate::net::{self, NetShared, NetState, net_clock_ms};

static HOOKED_BUILD: OnceLock<VerifiedBuild> = OnceLock::new();
static ORIGINAL_ITEM_UPDATE: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static GATE: Mutex<SceneStartGate> = Mutex::new(SceneStartGate::new());

type ItemUpdate = unsafe extern "system" fn(*mut c_void);

pub(super) fn install(build: VerifiedBuild) {
    if HOOKED_BUILD.set(build).is_err() {
        return;
    }
    #[expect(
        unsafe_code,
        reason = "hooks the script event item's update by address"
    )]
    // SAFETY: the RVA starts ScriptEventItem::Update (item) in the verified build (see docs
    // off-hand/cutscenes.md) and the replacement has the same signature.
    unsafe {
        hook_address(
            "ScriptEventItem::Update",
            (build.module_base() + u64::from(SCENE_ITEM_UPDATE_RVA)) as usize,
            item_update_detour as *const () as *mut c_void,
            &ORIGINAL_ITEM_UPDATE,
        );
    }
}

pub(super) fn observe_host(build: &VerifiedBuild, view: &NetShared) {
    let mut gate = GATE.lock().unwrap_or_else(PoisonError::into_inner);
    if guest_waits_for_host(build, view) {
        gate.observe_host(partner_in_cutscene(view));
    } else {
        gate.reset();
    }
}

#[expect(
    unsafe_code,
    reason = "calls the original item update through the trampoline MinHook built"
)]
extern "system" fn item_update_detour(item: *mut c_void) {
    if panic::catch_unwind(|| holds_scene_start(item as u64)).unwrap_or(false) {
        return;
    }
    let original = ORIGINAL_ITEM_UPDATE.load(Ordering::Acquire);
    if original.is_null() {
        return;
    }
    // SAFETY: the trampoline continues into the original update, which has this signature.
    unsafe { core::mem::transmute::<*mut c_void, ItemUpdate>(original)(item) };
}

fn guest_waits_for_host(build: &VerifiedBuild, view: &NetShared) -> bool {
    if view.state != NetState::Connected
        || FollowRole::from_slot(view.local_slot) != FollowRole::Guest
        || !view.partner_in_play()
    {
        return false;
    }
    let game = Replicant::new(InProcessMemory::new(build));
    let local_phase = game
        .current_phase()
        .ok()
        .and_then(|name| PhaseEntry::index_of(&name))
        .and_then(|index| u16::try_from(index).ok());
    view.partner_state()
        .is_some_and(|remote| Some(remote.state.phase) == local_phase)
}

fn holds_scene_start(item: u64) -> bool {
    let Some(build) = HOOKED_BUILD.get() else {
        return false;
    };
    let Some(net) = net::handle() else {
        return false;
    };
    let memory = InProcessMemory::new(build);
    let state = memory
        .read_i32(item + SCENE_ITEM_STATE_OFFSET)
        .unwrap_or(-1)
        .cast_unsigned();
    let cut_loaded = memory
        .read_u8(build.module_base() + u64::from(EVENT_CUT_LOADED_BYTE_RVA))
        .is_ok_and(|byte| byte != 0);
    if !cut_loaded || (state != SCENE_ITEM_STATE_NOT_STARTED && state != SCENE_ITEM_STATE_STARTING)
    {
        return false;
    }
    let view = net.view();
    if !guest_waits_for_host(build, &view) {
        return false;
    }
    let verdict = GATE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .ask(net_clock_ms());
    let scene_id = memory
        .read_i32(item + SCENE_ITEM_SCENE_ID_OFFSET)
        .unwrap_or(-1);
    match verdict {
        StartVerdict::Hold { first } => {
            if first {
                log_line(&format!(
                    "scene hold: your scene {scene_id} waits for the host to enter its own (item state {state:#x}, at most {SCENE_START_HOLD_TIMEOUT_MS} ms)"
                ));
            }
            true
        }
        StartVerdict::Start { waited_ms } => {
            if waited_ms > 0 {
                log_line(&format!(
                    "scene hold: the host is in its scene, your scene {scene_id} starts after {waited_ms} ms"
                ));
            }
            false
        }
        StartVerdict::StartAfterTimeout { waited_ms } => {
            log_line(&format!(
                "scene hold: timed out after {waited_ms} ms, your scene {scene_id} starts without the host"
            ));
            false
        }
    }
}
