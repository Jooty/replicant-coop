use serde::{Deserialize, Serialize};

use crate::wire::{
    BULLET_STYLE_FLOATS, BULLET_TRANSFORM_FLOATS, MAX_BULLET_AXIS_ABS, MAX_BULLET_INSTANCE,
    MAX_BULLET_STYLE_ABS, MAX_POSITION_ABS, WireError,
};

pub const MAX_SHOT_CUE_ID: u32 = 0x00FF_FFFF;
pub const SHOT_SOUND_LEVELS: usize = 2;
pub const MAX_SHOT_SOUND_LEVEL_ABS: f32 = 1000.0;

/// What a shot's own code made besides its look: a picture or a sound, and whether it lasts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShotCueKind {
    /// An effect bound to the shot that moves with it until the shot or its code ends it.
    FollowingEffect,
    /// An effect made once where the shot was, such as an impact or a puff of its trail.
    OneShotEffect,
    /// A sound played once where the shot was.
    OneShotSound,
    /// A sound that plays on and moves with the shot until it is stopped.
    LastingSound,
    EndLastingSound,
    EndFollowingEffects,
}

/// One effect or sound a shot made on its owner's side, for the copy of that shot to make too.
/// `instance` names the shot as its flight messages do; `None` when no copy was ever told of it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShotCue {
    pub instance: Option<u16>,
    pub kind: ShotCueKind,
    /// The game's effect id or sound id; 0 for the two end kinds.
    pub id: u32,
    /// Where the owner made it, 3x4 rows with the translation in the last column.
    pub transform: [f32; BULLET_TRANSFORM_FLOATS],
    /// From the shot's position to the cue's, on the owner's side.
    pub offset: [f32; 3],
    /// An effect's colour and scale, as a shot's look styles are.
    pub style: [f32; BULLET_STYLE_FLOATS],
    /// A sound's two level arguments, as the owner's game passed them.
    pub sound_levels: [f32; SHOT_SOUND_LEVELS],
}

impl ShotCueKind {
    pub fn is_effect(self) -> bool {
        matches!(self, Self::FollowingEffect | Self::OneShotEffect)
    }

    pub fn is_sound(self) -> bool {
        matches!(self, Self::OneShotSound | Self::LastingSound)
    }
}

impl ShotCue {
    pub fn translation(&self) -> [f32; 3] {
        [self.transform[3], self.transform[7], self.transform[11]]
    }

    pub fn validate(&self) -> Result<(), WireError> {
        if self
            .instance
            .is_some_and(|instance| instance > MAX_BULLET_INSTANCE)
        {
            return Err(WireError::OutOfRange {
                field: "shot cue instance",
            });
        }
        if self.id > MAX_SHOT_CUE_ID {
            return Err(WireError::OutOfRange {
                field: "shot cue id",
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
        let bad_offset = self
            .offset
            .iter()
            .any(|axis| !axis.is_finite() || axis.abs() > MAX_POSITION_ABS);
        if bad_transform || bad_offset {
            return Err(WireError::OutOfRange {
                field: "shot cue place",
            });
        }
        let bad_style = self
            .style
            .iter()
            .any(|value| !value.is_finite() || value.abs() > MAX_BULLET_STYLE_ABS);
        let bad_levels = self
            .sound_levels
            .iter()
            .any(|value| !value.is_finite() || value.abs() > MAX_SHOT_SOUND_LEVEL_ABS);
        if bad_style || bad_levels {
            return Err(WireError::OutOfRange {
                field: "shot cue style",
            });
        }
        Ok(())
    }
}
