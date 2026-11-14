use core::ffi::c_void;
use core::ptr;
use std::panic;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicPtr, Ordering};

use replicant_game::Replicant;
use replicant_game::offsets::LOADING_TEXT_PICKER_FUNCTION_RVA;
use replicant_game::player_name::{NAME_FIELD_LEN, diary_name};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::game_hooks::hook_address;
use crate::memory::InProcessMemory;
use crate::net::handle;

static ORIGINAL_PICKER: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static HOOKED_BUILD: OnceLock<VerifiedBuild> = OnceLock::new();

type PickAndFillText = unsafe extern "system" fn(*mut c_void, *mut i32, u8);

pub fn install(build: VerifiedBuild) {
    if HOOKED_BUILD.set(build).is_err() {
        return;
    }
    let address = (build.module_base() + u64::from(LOADING_TEXT_PICKER_FUNCTION_RVA)) as usize;
    #[expect(
        unsafe_code,
        reason = "hooks the loading screen text picker by address"
    )]
    // SAFETY: the RVA starts LoadingScreen::PickAndFillText (this, int cursor pointer, char gated;
    // void) in the verified build, and the replacement has the same signature.
    unsafe {
        hook_address(
            "LoadingScreen_PickAndFillText",
            address,
            picker_detour as *const () as *mut c_void,
            &ORIGINAL_PICKER,
        )
    };
}

#[expect(
    unsafe_code,
    reason = "calls the original picker through the trampoline MinHook built"
)]
extern "system" fn picker_detour(this: *mut c_void, cursor: *mut i32, gated: u8) {
    let original = ORIGINAL_PICKER.load(Ordering::Acquire);
    if original.is_null() {
        return;
    }
    let owner_name = panic::catch_unwind(put_partner_name_in_save).unwrap_or(None);
    // SAFETY: the trampoline continues into the original picker, which has this exact signature.
    unsafe { core::mem::transmute::<*mut c_void, PickAndFillText>(original)(this, cursor, gated) };
    if let Some(owner_name) = owner_name {
        let _ = panic::catch_unwind(|| put_back_owner_name(&owner_name));
    }
}

fn put_partner_name_in_save() -> Option<[u8; NAME_FIELD_LEN]> {
    let build = HOOKED_BUILD.get()?;
    let partner = handle()?.view().partner_name().map(str::to_owned);
    let partner_field = diary_name(partner.as_deref())?;
    let mut game = Replicant::new(InProcessMemory::new(build));
    let owner_name = game.save_name_field().ok()?;
    game.set_save_name_field(&partner_field).ok()?;
    log_line(&format!(
        "names: the loading screen diary uses {}",
        partner.unwrap_or_default()
    ));
    Some(owner_name)
}

fn put_back_owner_name(owner_name: &[u8; NAME_FIELD_LEN]) {
    let Some(build) = HOOKED_BUILD.get() else {
        return;
    };
    let mut game = Replicant::new(InProcessMemory::new(build));
    if game.set_save_name_field(owner_name).is_err() {
        log_line("names: could not put the save name back after the diary");
    }
}
