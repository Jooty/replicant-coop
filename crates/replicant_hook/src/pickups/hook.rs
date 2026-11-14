use core::ffi::c_void;
use core::ptr;
use std::panic;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicPtr, Ordering};

use replicant_game::offsets::{ITEM_MANAGER_ADD_FUNCTION_RVA, ITEM_MANAGER_RVA};
use replicant_game::pickups::{
    classify_add_caller, is_key_item, is_story_item, local_key_item_notice,
};
use replicant_proto::{ItemPickup, MAX_ITEM_COUNT, MAX_ITEM_ID, Message};
use windows_sys::Win32::System::Diagnostics::Debug::RtlCaptureStackBackTrace;

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::event_log::post_event;
use crate::game_hooks::hook_address;
use crate::net;
use crate::net::NetSharedExt;

static HOOKED_BUILD: OnceLock<VerifiedBuild> = OnceLock::new();
static ORIGINAL_ADD: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static SEEN_MANAGER: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

type AddItem = unsafe extern "system" fn(*mut c_void, i32, i32) -> i64;

pub fn install(build: VerifiedBuild) {
    if HOOKED_BUILD.set(build).is_err() {
        return;
    }
    #[expect(unsafe_code, reason = "hooks ItemManager::Add by its address")]
    // SAFETY: the RVA starts ItemManager::Add (manager, item id, count -> count added) in the
    // verified build, and the replacement has the same signature.
    unsafe {
        hook_address(
            "ItemManager::Add",
            (build.module_base() + u64::from(ITEM_MANAGER_ADD_FUNCTION_RVA)) as usize,
            add_item_detour as *const () as *mut c_void,
            &ORIGINAL_ADD,
        )
    };
    crate::pickups::item_calls::install(build);
    crate::pickups::quest_check::install(build);
    crate::pickups::turn_in::install(build);
}

#[expect(
    unsafe_code,
    reason = "calls the original ItemManager::Add through the trampoline MinHook built"
)]
pub fn add_item_for_partner(item_id: u16, count: u8) -> Option<i64> {
    let original = ORIGINAL_ADD.load(Ordering::Acquire);
    let build = HOOKED_BUILD.get()?;
    if original.is_null() {
        return None;
    }
    let mut manager = SEEN_MANAGER.load(Ordering::Acquire);
    if manager.is_null() {
        manager = (build.module_base() + u64::from(ITEM_MANAGER_RVA)) as usize as *mut c_void;
    }
    // SAFETY: the trampoline continues into the original Add with its exact signature; the
    // manager is the pointer the game itself passed, or the global manager object; this runs
    // on the game thread.
    Some(unsafe {
        core::mem::transmute::<*mut c_void, AddItem>(original)(
            manager,
            i32::from(item_id),
            i32::from(count),
        )
    })
}

#[expect(
    unsafe_code,
    reason = "calls the original ItemManager::Add through the trampoline MinHook built"
)]
extern "system" fn add_item_detour(manager: *mut c_void, item_id: i32, count: i32) -> i64 {
    let original = ORIGINAL_ADD.load(Ordering::Acquire);
    if original.is_null() {
        return 0;
    }
    SEEN_MANAGER.store(manager, Ordering::Release);
    let caller_rva = caller_rva_of_this_detour();
    // SAFETY: the trampoline continues into the original Add, which has this exact signature.
    let added =
        unsafe { core::mem::transmute::<*mut c_void, AddItem>(original)(manager, item_id, count) };
    if added > 0 {
        let _ = panic::catch_unwind(|| report_pickup(item_id, added, caller_rva));
        if let Ok(item_id) = u16::try_from(item_id)
            && let Some(count) = crate::pickups::item_calls::local_count(item_id)
        {
            let _ = panic::catch_unwind(|| {
                crate::pickups::shared_items::announce_local_count(item_id, count);
            });
        }
        let _ =
            panic::catch_unwind(|| crate::containers::note_item_added(item_id, added, caller_rva));
    }
    added
}

fn report_pickup(item_id: i32, added: i64, caller_rva: u32) {
    let Some(net) = net::handle() else {
        return;
    };
    let view = net.view();
    if !view.partner_in_play() {
        return;
    }
    let (Ok(item_id), Ok(count)) = (
        u16::try_from(item_id),
        u8::try_from(added.min(i64::from(MAX_ITEM_COUNT))),
    ) else {
        return;
    };
    if item_id > MAX_ITEM_ID {
        return;
    }
    let count = if is_story_item(item_id) {
        crate::pickups::item_calls::local_count(item_id).map_or(count, |total| total.max(count))
    } else {
        count
    };
    let pickup = ItemPickup {
        item_id,
        count,
        source: classify_add_caller(caller_rva),
    };
    net.send_relayed(Message::Pickup(pickup));
    log_line(&format!(
        "pickups: sent item id {item_id} x{count}, caller rva {caller_rva:#x}, source {:?}, key item {}",
        pickup.source,
        is_key_item(item_id)
    ));
    if let Some(name) = view.partner_name().map(str::to_owned)
        && let Some(notice) = local_key_item_notice(&name, pickup)
    {
        post_event(&notice);
    }
}

#[expect(
    unsafe_code,
    reason = "walks one stack frame to find who called the detour"
)]
fn caller_rva_of_this_detour() -> u32 {
    let Some(build) = HOOKED_BUILD.get() else {
        return 0;
    };
    let mut frames = [ptr::null_mut::<c_void>(); 1];
    // SAFETY: the buffer holds the one frame asked for; skipping one frame passes the detour
    // itself, so the entry is the return address inside the game function that called Add.
    let captured = unsafe { RtlCaptureStackBackTrace(1, 1, frames.as_mut_ptr(), ptr::null_mut()) };
    if captured == 0 {
        return 0;
    }
    let address = frames[0] as usize as u64;
    u32::try_from(address.wrapping_sub(build.module_base())).unwrap_or(0)
}
