use core::ffi::{CStr, c_void};
use core::ptr;
use std::panic;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicPtr, Ordering};

use replicant_game::offsets::{PLAYABLE_SEED_NAME_FUNCTION_RVA, SEED_NAME_TABLE_RVA};
use replicant_game::puppet::{SEED_NAME_TABLE_STRIDE, body_info, own_seed_swap};
use replicant_game::stay_together::FollowRole;
use replicant_proto::GUEST_BODY;

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::game_hooks::hook_address;
use crate::net;
use crate::player_body::real_body::note_seed_in_use;

static ORIGINAL_GETTER: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static HOOKED: OnceLock<VerifiedBuild> = OnceLock::new();

type GetSeedName = unsafe extern "system" fn(*mut c_void) -> *const u8;

pub fn install(build: VerifiedBuild) {
    if HOOKED.set(build).is_err() {
        return;
    }
    let info = body_info(GUEST_BODY);
    log_line(&format!(
        "player body: as a guest seeds {:?} become {} at phase load ({})",
        info.own_seed_replaces, info.own_seed_name, info.own_body_changes
    ));
    let address = (build.module_base() + u64::from(PLAYABLE_SEED_NAME_FUNCTION_RVA)) as usize;
    #[expect(unsafe_code, reason = "hooks the playable seed name getter by address")]
    // SAFETY: the RVA starts the seed name getter (this, returns a NUL-terminated name; never
    // writes the save) in the verified build, and the replacement has the same signature.
    unsafe {
        hook_address(
            "PlayableSeedName",
            address,
            seed_name_detour as *const () as *mut c_void,
            &ORIGINAL_GETTER,
        )
    };
}

#[expect(
    unsafe_code,
    reason = "calls the original seed name getter through the trampoline MinHook built"
)]
extern "system" fn seed_name_detour(this: *mut c_void) -> *const u8 {
    let original = ORIGINAL_GETTER.load(Ordering::Acquire);
    if original.is_null() {
        return ptr::null();
    }
    // SAFETY: the trampoline continues into the original getter, which has this exact signature.
    let returned = unsafe { core::mem::transmute::<*mut c_void, GetSeedName>(original)(this) };
    panic::catch_unwind(|| own_body_seed(returned))
        .ok()
        .flatten()
        .unwrap_or(returned)
}

#[expect(
    unsafe_code,
    reason = "reads the game's NUL-terminated seed name strings"
)]
fn own_body_seed(returned: *const u8) -> Option<*const u8> {
    let build = HOOKED.get()?;
    if returned.is_null() {
        return None;
    }
    // SAFETY: the getter returns a pointer into the game's static seed name table.
    let returned_name = unsafe { CStr::from_ptr(returned.cast()) }.to_str().ok()?;
    let role = net::handle().map_or(FollowRole::Unknown, |net| {
        FollowRole::from_slot(net.view().local_slot)
    });
    let Some(swap) = own_seed_swap(role, returned_name) else {
        note_real_body(returned_name, role, false);
        return None;
    };
    let table = build.module_base() + u64::from(SEED_NAME_TABLE_RVA);
    let replacement = (table + u64::from(swap.table_index) * SEED_NAME_TABLE_STRIDE) as *const u8;
    // SAFETY: the table entry is a 16-byte slot of the same static table the getter returns from.
    let replacement_name = unsafe { CStr::from_ptr(replacement.cast()) }
        .to_str()
        .ok()?;
    if replacement_name != swap.to {
        log_line(&format!(
            "player body: table entry {} reads {replacement_name}, not {}; seed unchanged",
            swap.table_index, swap.to
        ));
        note_real_body(returned_name, role, false);
        return None;
    }
    log_line(&format!(
        "player body: seed {} instead of {}",
        swap.to, swap.from
    ));
    note_real_body(swap.to, role, true);
    crate::kaine::log_her_level_before_player_is_built(build);
    Some(replacement)
}

fn note_real_body(seed_name: &str, role: FollowRole, by_swap: bool) {
    match note_seed_in_use(seed_name, by_swap) {
        Some(real) => log_line(&format!(
            "body: phase load {} wears {:?} (seed {seed_name}, role {role:?}, swapped {by_swap})",
            real.phase_loads, real.body
        )),
        None => log_line(&format!(
            "body: phase load wears seed {seed_name}, which has no known pack, nothing announced"
        )),
    }
}
