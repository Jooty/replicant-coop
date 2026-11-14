use crate::fight_sync::camera_follow::FixedCameraPose;
use crate::memory::GameMemory;

pub const FIXED_CAMERA_RVA: u32 = 0x32C_4260;
pub const FIXED_CAMERA_EYE_LERP_END_OFFSET: u64 = 0x5C0;
pub const FIXED_CAMERA_LOOK_POINT_OFFSET: u64 = 0x3A0;
pub const FIXED_CAMERA_LOOK_LERP_END_OFFSET: u64 = 0x530;
pub const FIXED_CAMERA_LOOK_ACTOR_GENERATION_OFFSET: u64 = 0x4E4;

/// What the fixed camera (index 8) holds: the eye and look point it is heading to (the lerp
/// ends that `SetEye` 0x340F50 and `SetLookDirection` 0x340D60 write at once), the look point
/// its last refresh drew (+0x3A0, late by a frame after a status switch), and the generation of
/// the actor its look follows (0 for none).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FixedCameraReading {
    pub eye_end: [f32; 3],
    pub look_end: [f32; 3],
    pub look_now: [f32; 3],
    pub look_actor_generation: i32,
}

impl FixedCameraReading {
    pub fn follows_an_actor(&self) -> bool {
        self.look_actor_generation != 0
    }

    /// The pose to send: the lerp ends, or the drawn look point while the look follows an
    /// actor (then the lerp end is not where it looks).
    pub fn pose(&self) -> FixedCameraPose {
        FixedCameraPose {
            eye: self.eye_end,
            look_at: if self.follows_an_actor() {
                self.look_now
            } else {
                self.look_end
            },
        }
    }
}

pub fn read_fixed_camera_reading<M: GameMemory>(memory: &M) -> Option<FixedCameraReading> {
    let camera = memory.module_base() + u64::from(FIXED_CAMERA_RVA);
    let point = |at: u64| -> Option<[f32; 3]> {
        let point = [
            memory.read_f32(at).ok()?,
            memory.read_f32(at + 4).ok()?,
            memory.read_f32(at + 8).ok()?,
        ];
        point.iter().all(|axis| axis.is_finite()).then_some(point)
    };
    Some(FixedCameraReading {
        eye_end: point(camera + FIXED_CAMERA_EYE_LERP_END_OFFSET)?,
        look_end: point(camera + FIXED_CAMERA_LOOK_LERP_END_OFFSET)?,
        look_now: point(camera + FIXED_CAMERA_LOOK_POINT_OFFSET)?,
        look_actor_generation: memory
            .read_i32(camera + FIXED_CAMERA_LOOK_ACTOR_GENERATION_OFFSET)
            .ok()?,
    })
}

pub fn read_fixed_camera_pose<M: GameMemory>(memory: &M) -> Option<FixedCameraPose> {
    read_fixed_camera_reading(memory).map(|reading| reading.pose())
}

pub fn fixed_camera_text(reading: Option<FixedCameraReading>) -> String {
    let Some(reading) = reading else {
        return "fixed camera unreadable".to_owned();
    };
    let point = |p: [f32; 3]| format!("{:.3} {:.3} {:.3}", p[0], p[1], p[2]);
    format!(
        "fixed camera eye {} look {} (look now {}, look actor {})",
        point(reading.eye_end),
        point(reading.look_end),
        point(reading.look_now),
        if reading.follows_an_actor() {
            "yes"
        } else {
            "none"
        }
    )
}
