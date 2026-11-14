use serde::{Deserialize, Serialize};

use crate::wire::{BulletFlight, EnemyId, WireError};

pub const MAX_BULLET_SPRITE_KIND: u8 = 1;
pub const MAX_BULLET_SPRITE_SIZE: f32 = 1000.0;
pub const MAX_BULLET_SPRITE_ALPHA: f32 = 16.0;
pub const MAX_BULLET_SPRITE_ANGLE_ABS: f32 = 1000.0;
pub const MAX_SHOT_SCALE_GROWTH_PER_SECOND: f32 = 1000.0;

/// What the game's bullet sprite batch needs, besides a place, to draw one sprite-drawn bullet.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BulletSprite {
    pub kind: u8,
    pub size: f32,
    pub alpha: f32,
    /// Radians.
    pub angle: f32,
}

/// One step of a bullet fired by one of the host's enemies, for the guest to draw. The guest's
/// copy never fires; damage from the bullet reaches the guest as a `PlayerHit` from the host.
/// A sprite-drawn bullet carries `sprite` on every step and never a look.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnemyBulletFlight {
    pub enemy: EnemyId,
    pub flight: BulletFlight,
    pub sprite: Option<BulletSprite>,
    /// How fast the shot's effects grow, as a share of their scale now, per second: the game
    /// widens some ordinary bullets each frame and scales their effects with them. 0 otherwise.
    pub scale_growth_per_second: f32,
}

impl BulletSprite {
    pub fn validate(&self) -> Result<(), WireError> {
        if self.kind > MAX_BULLET_SPRITE_KIND {
            return Err(WireError::OutOfRange {
                field: "bullet sprite kind",
            });
        }
        if !(0.0..=MAX_BULLET_SPRITE_SIZE).contains(&self.size) {
            return Err(WireError::OutOfRange {
                field: "bullet sprite size",
            });
        }
        if !(0.0..=MAX_BULLET_SPRITE_ALPHA).contains(&self.alpha) {
            return Err(WireError::OutOfRange {
                field: "bullet sprite alpha",
            });
        }
        if !self.angle.is_finite() || self.angle.abs() > MAX_BULLET_SPRITE_ANGLE_ABS {
            return Err(WireError::OutOfRange {
                field: "bullet sprite angle",
            });
        }
        Ok(())
    }
}

impl EnemyBulletFlight {
    pub fn validate(&self) -> Result<(), WireError> {
        self.enemy.validate()?;
        self.flight.validate()?;
        if !(0.0..=MAX_SHOT_SCALE_GROWTH_PER_SECOND).contains(&self.scale_growth_per_second) {
            return Err(WireError::OutOfRange {
                field: "shot scale growth",
            });
        }
        match &self.sprite {
            Some(sprite) => sprite.validate(),
            None => Ok(()),
        }
    }
}
