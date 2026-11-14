use crate::carts::{
    FIRST_PARKED_CART_STATE, FIRST_RUNNING_CART_STATE, LAST_PARKED_CART_STATE,
    LAST_RUNNING_CART_STATE,
};

/// `CActorTruck` +0x14D84: 1 to 4 parked at one end of its rail (each plays its own still
/// motion), 5 to 8 running along it (a baked run motion moves the cart), anything else unset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartState {
    Unset,
    Parked(u8),
    Running(u8),
}

pub fn cart_state(raw: i32) -> CartState {
    match u8::try_from(raw) {
        Ok(state) if (FIRST_PARKED_CART_STATE..=LAST_PARKED_CART_STATE).contains(&state) => {
            CartState::Parked(state)
        }
        Ok(state) if (FIRST_RUNNING_CART_STATE..=LAST_RUNNING_CART_STATE).contains(&state) => {
            CartState::Running(state)
        }
        _ => CartState::Unset,
    }
}

/// The run a parked cart starts when a player boards it (`CActorTruck` message 0x11).
pub fn run_state_from(parked: u8) -> Option<u8> {
    (FIRST_PARKED_CART_STATE..=LAST_PARKED_CART_STATE)
        .contains(&parked)
        .then(|| parked + (FIRST_RUNNING_CART_STATE - FIRST_PARKED_CART_STATE))
}

/// Where a run parks the cart once its motion ends (`CActorTruck::Update`): 5 and 6 swap
/// ends 1 and 2, 7 and 8 swap ends 3 and 4.
pub fn parked_state_after(running: u8) -> Option<u8> {
    match running {
        5 => Some(2),
        6 => Some(1),
        7 => Some(4),
        8 => Some(3),
        _ => None,
    }
}
