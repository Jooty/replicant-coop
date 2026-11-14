use serde::{Deserialize, Serialize};

use crate::wire::{MAX_POSITION_ABS, WireError};

/// A bomb-breakable wall that started to break on the sender's game: its map layout id when
/// the map knows one, and where it stands, so the receiver can find its own copy either way.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WallBroken {
    pub phase: u16,
    pub layout_id: Option<u16>,
    pub position: [f32; 3],
}

/// One of the sender's own bombs went off here; the receiver plays the blast and harms nothing.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BombBlast {
    pub phase: u16,
    pub position: [f32; 3],
}

impl WallBroken {
    pub fn validate(&self) -> Result<(), WireError> {
        validate_position(self.position, "wall position")
    }
}

impl BombBlast {
    pub fn validate(&self) -> Result<(), WireError> {
        validate_position(self.position, "bomb blast position")
    }
}

fn validate_position(position: [f32; 3], field: &'static str) -> Result<(), WireError> {
    let outside = position
        .iter()
        .any(|axis| !axis.is_finite() || axis.abs() > MAX_POSITION_ABS);
    if outside {
        return Err(WireError::OutOfRange { field });
    }
    Ok(())
}
