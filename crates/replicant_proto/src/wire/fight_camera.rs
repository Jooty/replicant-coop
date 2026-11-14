use serde::{Deserialize, Serialize};

use crate::wire::{MAX_POSITION_ABS, WireError};

pub const MAX_FIGHT_CAMERA_INDEX: i32 = 31;
pub const MAX_FIGHT_CAMERA_BLEND_SECONDS: f32 = 60.0;
pub const MAX_FIGHT_CAMERA_BLEND_KIND: i32 = 3;

/// The fixed camera's eye and the point it looks at, in world units.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FightCameraPose {
    pub eye: [f32; 3],
    pub look_at: [f32; 3],
}

/// The host's active camera changed in a fight window. A higher `seq` replaces a lower one;
/// `natives_before` counts the host's `_ChangeCamera` calls in the window before this switch.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FightCamera {
    pub phase: u16,
    pub seq: u32,
    pub natives_before: u32,
    pub camera: i32,
    pub blend_seconds: f32,
    pub blend_kind: i32,
    pub pose: Option<FightCameraPose>,
}

/// A fixed camera that follows a body: eye and look offsets from the body on the floor plane
/// (x, z) and their heights as they are.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FightCameraBodyOffset {
    pub eye_offset_xz: [f32; 2],
    pub eye_height: f32,
    pub look_offset_xz: [f32; 2],
    pub look_height: f32,
}

/// The host's fixed camera moved while it stayed active: a pose only, never a switch. A higher
/// `seq` replaces a lower one. With `follows_body` the camera follows the host's body and the
/// guest builds it around its own; without it `pose` is taken as it is.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FightCameraAim {
    pub phase: u16,
    pub seq: u32,
    pub pose: FightCameraPose,
    pub follows_body: Option<FightCameraBodyOffset>,
}

impl FightCameraPose {
    pub fn validate(&self) -> Result<(), WireError> {
        let far = self
            .eye
            .iter()
            .chain(&self.look_at)
            .any(|axis| !axis.is_finite() || axis.abs() > MAX_POSITION_ABS);
        if far {
            return Err(WireError::OutOfRange { field: "pose" });
        }
        Ok(())
    }
}

impl FightCamera {
    pub fn validate(&self) -> Result<(), WireError> {
        if !(0..=MAX_FIGHT_CAMERA_INDEX).contains(&self.camera) {
            return Err(WireError::OutOfRange { field: "camera" });
        }
        if !(0.0..=MAX_FIGHT_CAMERA_BLEND_SECONDS).contains(&self.blend_seconds) {
            return Err(WireError::OutOfRange {
                field: "blend_seconds",
            });
        }
        if !(0..=MAX_FIGHT_CAMERA_BLEND_KIND).contains(&self.blend_kind) {
            return Err(WireError::OutOfRange {
                field: "blend_kind",
            });
        }
        self.pose.map_or(Ok(()), |pose| pose.validate())
    }
}

impl FightCameraBodyOffset {
    pub fn validate(&self) -> Result<(), WireError> {
        let far = self
            .eye_offset_xz
            .iter()
            .chain(&self.look_offset_xz)
            .chain([&self.eye_height, &self.look_height])
            .any(|axis| !axis.is_finite() || axis.abs() > MAX_POSITION_ABS);
        if far {
            return Err(WireError::OutOfRange {
                field: "follows_body",
            });
        }
        Ok(())
    }
}

impl FightCameraAim {
    pub fn validate(&self) -> Result<(), WireError> {
        self.pose.validate()?;
        self.follows_body.map_or(Ok(()), |offset| offset.validate())
    }
}
