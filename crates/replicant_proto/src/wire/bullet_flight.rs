use serde::{Deserialize, Serialize};

use crate::wire::{MAX_POSITION_ABS, WireError};

pub const MAX_BULLET_INSTANCE: u16 = 4095;
pub const BULLET_EFFECT_SLOTS: usize = 4;
pub const BULLET_TRANSFORM_FLOATS: usize = 12;
pub const BULLET_STYLE_FLOATS: usize = 8;
pub const MAX_BULLET_EFFECT_ID: i32 = 0x3FFF;
pub const MAX_BULLET_AXIS_ABS: f32 = 4096.0;
pub const MAX_BULLET_SPEED_ABS: f32 = 10_000.0;
pub const MAX_BULLET_STYLE_ABS: f32 = 10_000.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BulletLook {
    pub effect_ids: [i32; BULLET_EFFECT_SLOTS],
    pub styles: [[f32; BULLET_STYLE_FLOATS]; BULLET_EFFECT_SLOTS],
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BulletFlight {
    pub instance: u16,
    pub transform: [f32; BULLET_TRANSFORM_FLOATS],
    pub velocity: [f32; 3],
    pub look: Option<BulletLook>,
    pub ended: bool,
}

impl BulletLook {
    pub fn validate(&self) -> Result<(), WireError> {
        if self.effect_ids.iter().any(|id| *id > MAX_BULLET_EFFECT_ID) {
            return Err(WireError::OutOfRange {
                field: "bullet effect id",
            });
        }
        let outside = self
            .styles
            .iter()
            .flatten()
            .any(|value| !value.is_finite() || value.abs() > MAX_BULLET_STYLE_ABS);
        if outside {
            return Err(WireError::OutOfRange {
                field: "bullet style",
            });
        }
        Ok(())
    }
}

impl BulletFlight {
    pub fn translation(&self) -> [f32; 3] {
        [self.transform[3], self.transform[7], self.transform[11]]
    }

    pub fn validate(&self) -> Result<(), WireError> {
        if self.instance > MAX_BULLET_INSTANCE {
            return Err(WireError::OutOfRange {
                field: "bullet instance",
            });
        }
        let bad_transform = self.transform.iter().enumerate().any(|(index, value)| {
            let limit = if index % 4 == 3 {
                MAX_POSITION_ABS
            } else {
                MAX_BULLET_AXIS_ABS
            };
            !value.is_finite() || value.abs() > limit
        });
        if bad_transform {
            return Err(WireError::OutOfRange {
                field: "bullet transform",
            });
        }
        let bad_velocity = self
            .velocity
            .iter()
            .any(|axis| !axis.is_finite() || axis.abs() > MAX_BULLET_SPEED_ABS);
        if bad_velocity {
            return Err(WireError::OutOfRange {
                field: "bullet velocity",
            });
        }
        match &self.look {
            Some(look) => look.validate(),
            None => Ok(()),
        }
    }
}
