use crate::memory::GameMemory;
use crate::side_view::CHANGE_CAMERA_NATIVE_RVA;
use crate::story_jump::{
    NATIVE_ARG_FIRST_OFFSET_BYTES, NATIVE_ARG_INTEGER_TYPE_TAG, NATIVE_ARG_REAL_TYPE_TAG,
    NATIVE_ARG_STRIDE_BYTES, NATIVE_ARG_VALUE_OFFSET_BYTES, NATIVE_ARGS_HEADER_BYTES,
};

pub const SET_FIXED_CAMERA_NATIVE_RVA: u32 = 0x6F_93E0;
pub const SET_FREE_CAMERA_DIRECTION_FIXED_NATIVE_RVA: u32 = 0x6F_AA20;
pub const SET_FREE_CAMERA_DISTANCE_OFFSET_NATIVE_RVA: u32 = 0x6F_AF20;
pub const RESET_FREE_CAMERA_DIRECTION_WITH_ACTOR_GROUP_NATIVE_RVA: u32 = 0x6F_2820;

const MIN_FORWARD_LENGTH: f32 = 1.0e-4;

/// The camera natives a fight script calls to set its angle, numbered for the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptedCameraNative {
    SetFixedCamera,
    ChangeCamera,
    SetFreeCameraDirectionFixed,
    SetFreeCameraDistanceOffset,
    ResetFreeCameraDirectionWithActorGroup,
}

impl ScriptedCameraNative {
    pub const ALL: [Self; 5] = [
        Self::SetFixedCamera,
        Self::ChangeCamera,
        Self::SetFreeCameraDirectionFixed,
        Self::SetFreeCameraDistanceOffset,
        Self::ResetFreeCameraDirectionWithActorGroup,
    ];

    pub fn code(self) -> u8 {
        match self {
            Self::SetFixedCamera => 1,
            Self::ChangeCamera => 2,
            Self::SetFreeCameraDirectionFixed => 3,
            Self::SetFreeCameraDistanceOffset => 4,
            Self::ResetFreeCameraDirectionWithActorGroup => 5,
        }
    }

    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|native| native.code() == code)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::SetFixedCamera => "_SetFixedCamera",
            Self::ChangeCamera => "_ChangeCamera",
            Self::SetFreeCameraDirectionFixed => "_SetFreeCameraDirectionFixed",
            Self::SetFreeCameraDistanceOffset => "_SetFreeCameraDistanceOffset",
            Self::ResetFreeCameraDirectionWithActorGroup => {
                "_ResetFreeCameraDirectionWithActorGroup"
            }
        }
    }

    pub fn rva(self) -> u32 {
        match self {
            Self::SetFixedCamera => SET_FIXED_CAMERA_NATIVE_RVA,
            Self::ChangeCamera => CHANGE_CAMERA_NATIVE_RVA,
            Self::SetFreeCameraDirectionFixed => SET_FREE_CAMERA_DIRECTION_FIXED_NATIVE_RVA,
            Self::SetFreeCameraDistanceOffset => SET_FREE_CAMERA_DISTANCE_OFFSET_NATIVE_RVA,
            Self::ResetFreeCameraDirectionWithActorGroup => {
                RESET_FREE_CAMERA_DIRECTION_WITH_ACTOR_GROUP_NATIVE_RVA
            }
        }
    }

    pub fn argument_count(self) -> usize {
        match self {
            Self::SetFixedCamera => 6,
            Self::ChangeCamera => 3,
            Self::SetFreeCameraDirectionFixed => 11,
            Self::SetFreeCameraDistanceOffset => 5,
            Self::ResetFreeCameraDirectionWithActorGroup => 4,
        }
    }
}

/// The camera's look direction, column 2 of the published world matrix, as a unit vector.
pub fn forward_of_world_matrix(matrix: &[f32; 16]) -> Option<[f32; 3]> {
    normalized([matrix[2], matrix[6], matrix[10]])
}

/// Yaw about the up axis (0 looks along +Z, 90 along +X) and pitch (positive looks up), in degrees.
pub fn look_angles_degrees(forward: [f32; 3]) -> (f32, f32) {
    let yaw = forward[0].atan2(forward[2]).to_degrees();
    let pitch = forward[1].atan2(forward[0].hypot(forward[2])).to_degrees();
    (yaw, pitch)
}

pub fn angle_between_degrees(a: [f32; 3], b: [f32; 3]) -> f32 {
    let (Some(a), Some(b)) = (normalized(a), normalized(b)) else {
        return f32::NAN;
    };
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    dot.clamp(-1.0, 1.0).acos().to_degrees()
}

pub fn direction_text(forward: Option<[f32; 3]>) -> String {
    forward.map_or_else(
        || "an unreadable direction".to_owned(),
        |forward| {
            let (yaw, pitch) = look_angles_degrees(forward);
            format!("yaw {yaw:.1} pitch {pitch:.1}")
        },
    )
}

/// The numbers a native was called with, read from its argument block (`None` for a slot that
/// is not a number). `arguments` is the native's own argument pointer.
pub fn read_native_numbers<M: GameMemory>(
    memory: &M,
    arguments: u64,
    count: usize,
) -> Vec<Option<f64>> {
    (0..count)
        .map(|index| {
            let slot = arguments
                + (NATIVE_ARGS_HEADER_BYTES
                    + NATIVE_ARG_FIRST_OFFSET_BYTES
                    + index * NATIVE_ARG_STRIDE_BYTES) as u64;
            let value = slot + NATIVE_ARG_VALUE_OFFSET_BYTES as u64;
            match memory.read_i32(slot).ok()?.cast_unsigned() {
                NATIVE_ARG_REAL_TYPE_TAG => memory.read_f64(value).ok(),
                NATIVE_ARG_INTEGER_TYPE_TAG => memory.read_i32(value).ok().map(f64::from),
                _ => None,
            }
        })
        .collect()
}

pub fn camera_call_line(
    native: ScriptedCameraNative,
    numbers: &[Option<f64>],
    forward_before: Option<[f32; 3]>,
    active_camera: Option<i32>,
    in_fight_window: bool,
) -> String {
    let arguments: Vec<String> = numbers
        .iter()
        .map(|number| number.map_or_else(|| "?".to_owned(), |number| format!("{number}")))
        .collect();
    format!(
        "scripted camera: {}({}) ran {}; the camera looked {} with camera {} active",
        native.name(),
        arguments.join(","),
        if in_fight_window {
            "in a fight window"
        } else {
            "outside a fight window"
        },
        direction_text(forward_before),
        active_camera.map_or_else(|| "?".to_owned(), |index| index.to_string())
    )
}

fn normalized(vector: [f32; 3]) -> Option<[f32; 3]> {
    let length = (vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2]).sqrt();
    (length.is_finite() && length > MIN_FORWARD_LENGTH)
        .then(|| [vector[0] / length, vector[1] / length, vector[2] / length])
}
