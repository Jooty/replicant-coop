use serde::{Deserialize, Serialize};

use crate::wire::{MAX_POSITION_ABS, MAX_WARP_POINT_ID, WireError};

/// The guest picked a floor on a lift panel: `point` is the floor picked, `origin` the warp point
/// the panel opened on, `position` where the guest stood. The host runs the same pick.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LiftCall {
    pub phase: u16,
    pub point: u16,
    pub origin: u16,
    pub position: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiftCallOutcome {
    Riding,
    HostBusy,
    OtherMap,
    NotALift,
    AnotherRideRunning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiftCallAnswer {
    pub phase: u16,
    pub point: u16,
    pub outcome: LiftCallOutcome,
}

impl LiftCall {
    pub fn validate(&self) -> Result<(), WireError> {
        if self.point > MAX_WARP_POINT_ID || self.origin > MAX_WARP_POINT_ID {
            return Err(WireError::OutOfRange {
                field: "warp_point",
            });
        }
        if self
            .position
            .iter()
            .any(|axis| !axis.is_finite() || axis.abs() > MAX_POSITION_ABS)
        {
            return Err(WireError::OutOfRange { field: "position" });
        }
        Ok(())
    }
}

impl LiftCallAnswer {
    pub fn validate(&self) -> Result<(), WireError> {
        if self.point > MAX_WARP_POINT_ID {
            return Err(WireError::OutOfRange {
                field: "warp_point",
            });
        }
        Ok(())
    }
}
