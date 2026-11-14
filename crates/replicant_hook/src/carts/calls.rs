use core::ffi::c_void;

use replicant_game::carts::{
    CART_SET_STATE_FUNCTION_RVA, CartSighting, CartState, PLAYER_ON_CART_FLAG_OFFSET, cart_is_live,
    scan_cart_actors,
};
use replicant_game::{GameMemory, Replicant};

use crate::build_gate::VerifiedBuild;
use crate::door_sync::layout_id_of;
use crate::memory::InProcessMemory;

type SetCartState = unsafe extern "system" fn(*mut c_void, i32, u8) -> u64;

pub(super) fn carts_here(build: &VerifiedBuild) -> Vec<CartSighting> {
    let memory = InProcessMemory::new(build);
    scan_cart_actors(&memory, |actor| {
        u16::try_from(layout_id_of(build, actor as *mut c_void)).ok()
    })
}

pub(super) fn player_on_cart(build: &VerifiedBuild, player: u64) -> bool {
    player != 0
        && InProcessMemory::new(build)
            .read_i32(player + PLAYER_ON_CART_FLAG_OFFSET)
            .is_ok_and(|flag| flag == 1)
}

pub(crate) fn own_player_rides_a_cart(build: &VerifiedBuild) -> bool {
    let player = Replicant::new(InProcessMemory::new(build))
        .playable_actor()
        .unwrap_or(0);
    // A flag left set with no cart running (a cart removed under the rider) holds nothing back.
    player_on_cart(build, player)
        && carts_here(build)
            .iter()
            .any(|cart| matches!(cart.state, CartState::Running(_)))
}

#[expect(
    unsafe_code,
    reason = "calls CActorTruck's state setter by its address"
)]
pub(super) fn set_cart_state(build: &VerifiedBuild, actor: u64, state: u8) -> bool {
    if !cart_is_live(&InProcessMemory::new(build), actor) {
        return false;
    }
    let function = (build.module_base() + u64::from(CART_SET_STATE_FUNCTION_RVA)) as *const ();
    // SAFETY: the RVA starts CActorTruck's state setter (rcx cart, edx state, r8b "wait for the
    // start gate") in the verified build; the cart was just checked against the truck vtable and
    // this runs on the game thread. A zero third argument applies the state at once.
    unsafe {
        core::mem::transmute::<*const (), SetCartState>(function)(
            actor as *mut c_void,
            i32::from(state),
            0,
        );
    }
    true
}
