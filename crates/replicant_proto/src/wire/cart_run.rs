use serde::{Deserialize, Serialize};

use crate::wire::{
    MAX_CART_STATE, MAX_DOOR_LAYOUT_ID, MAX_POSITION_ABS, MIN_RUNNING_CART_STATE, WireError,
};

pub const MAX_CART_RUN_FRAME: f32 = 100_000.0;
pub const MAX_CART_RUN_RATE: f32 = 10.0;

/// How a ride ended: the game's end message cleared the rider's on-cart flag, the cart parked
/// at its far end with the flag still set, the cart's state left its run some other way, the
/// cart actor was removed under the rider, or the rider's map changed (or went) mid-ride.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CartRideEnd {
    OnCartFlagCleared,
    CartParked,
    StateLeftRun,
    CartGone,
    PhaseChanged,
}

/// Where the host's cart is along its run: its run motion's frame (60 a second at rate 1.0) and
/// the rate the host's game gave it this tick, or how the run ended.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum CartRunProgress {
    Moving { frame: f32, rate: f32 },
    Ended(CartRideEnd),
}

/// The progress of the ride the host started, named as the ride's start message named the cart
/// (`phase`, `layout_id`, `cart_position`, `run_state`). A higher `seq` replaces a lower one.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CartRun {
    pub phase: u16,
    pub layout_id: Option<u16>,
    pub cart_position: [f32; 3],
    pub run_state: u8,
    pub seq: u32,
    pub progress: CartRunProgress,
}

impl CartRun {
    pub fn validate(&self) -> Result<(), WireError> {
        if self
            .layout_id
            .is_some_and(|layout_id| layout_id > MAX_DOOR_LAYOUT_ID)
        {
            return Err(WireError::OutOfRange { field: "layout_id" });
        }
        if !(MIN_RUNNING_CART_STATE..=MAX_CART_STATE).contains(&self.run_state) {
            return Err(WireError::OutOfRange { field: "run_state" });
        }
        if self
            .cart_position
            .iter()
            .any(|axis| !axis.is_finite() || axis.abs() > MAX_POSITION_ABS)
        {
            return Err(WireError::OutOfRange {
                field: "cart_position",
            });
        }
        if let CartRunProgress::Moving { frame, rate } = self.progress {
            if !frame.is_finite() || !(0.0..=MAX_CART_RUN_FRAME).contains(&frame) {
                return Err(WireError::OutOfRange { field: "frame" });
            }
            if !rate.is_finite() || rate.abs() > MAX_CART_RUN_RATE {
                return Err(WireError::OutOfRange { field: "rate" });
            }
        }
        Ok(())
    }
}
