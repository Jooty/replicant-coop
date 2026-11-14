use crate::fight_sync::camera_aim_follow::CAMERA_AIM_MOVE_SENT_AGAIN_WORLD_UNITS;
use crate::fight_sync::camera_follow::FixedCameraPose;

pub const CAMERA_MOVE_PAST_THE_BODY_MOVE_IS_A_JUMP_WORLD_UNITS: f32 = 1.0;

/// A fixed-camera pose that follows a body: the eye and look point as offsets from the body on
/// the floor plane (x, z), and their heights as they are (the camera's height does not follow).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraBodyOffset {
    pub eye_offset_xz: [f32; 2],
    pub eye_height: f32,
    pub look_offset_xz: [f32; 2],
    pub look_height: f32,
}

impl CameraBodyOffset {
    pub fn from_pose_and_body(pose: FixedCameraPose, body: [f32; 3]) -> Self {
        Self {
            eye_offset_xz: [pose.eye[0] - body[0], pose.eye[2] - body[2]],
            eye_height: pose.eye[1],
            look_offset_xz: [pose.look_at[0] - body[0], pose.look_at[2] - body[2]],
            look_height: pose.look_at[1],
        }
    }

    /// The same camera built around another body.
    pub fn pose_around(&self, body: [f32; 3]) -> FixedCameraPose {
        FixedCameraPose {
            eye: [
                body[0] + self.eye_offset_xz[0],
                self.eye_height,
                body[2] + self.eye_offset_xz[1],
            ],
            look_at: [
                body[0] + self.look_offset_xz[0],
                self.look_height,
                body[2] + self.look_offset_xz[1],
            ],
        }
    }
}

/// Whether the fixed camera's move from `from` to `to` (each a pose and the host's body then) is
/// the camera following the body: same heights and view direction, a floor-plane move no longer
/// than the body's plus a slack, toward where the body went. A move with no body move keeps
/// `was_following`, since a following camera catches up after the body stops.
pub fn camera_follows_body(
    from: (FixedCameraPose, [f32; 3]),
    to: (FixedCameraPose, [f32; 3]),
    was_following: bool,
) -> bool {
    let ((from_pose, from_body), (to_pose, to_body)) = (from, to);
    let near = |a: f32, b: f32| (a - b).abs() <= CAMERA_AIM_MOVE_SENT_AGAIN_WORLD_UNITS;
    let same_heights =
        near(from_pose.eye[1], to_pose.eye[1]) && near(from_pose.look_at[1], to_pose.look_at[1]);
    let view = |pose: FixedCameraPose| {
        [
            pose.look_at[0] - pose.eye[0],
            pose.look_at[1] - pose.eye[1],
            pose.look_at[2] - pose.eye[2],
        ]
    };
    let (from_view, to_view) = (view(from_pose), view(to_pose));
    let same_view = (0..3).all(|axis| near(from_view[axis], to_view[axis]));
    if !same_heights || !same_view {
        return false;
    }
    let camera_move = [
        to_pose.eye[0] - from_pose.eye[0],
        to_pose.eye[2] - from_pose.eye[2],
    ];
    let body_move = [to_body[0] - from_body[0], to_body[2] - from_body[2]];
    let length = |v: [f32; 2]| v[0].hypot(v[1]);
    if length(camera_move)
        > length(body_move) + CAMERA_MOVE_PAST_THE_BODY_MOVE_IS_A_JUMP_WORLD_UNITS
    {
        return false;
    }
    let toward_the_body = camera_move[0] * body_move[0] + camera_move[1] * body_move[1];
    if toward_the_body > 0.0 {
        return true;
    }
    was_following
}

/// The per-second line's part about the local body: where it is, and the fixed camera's offset
/// from it (`following` says whether the camera is taken to follow it).
pub fn body_and_offset_text(
    body: Option<[f32; 3]>,
    pose: Option<FixedCameraPose>,
    following: bool,
) -> String {
    let Some(body) = body else {
        return "your body unreadable".to_owned();
    };
    let place = format!("your body {:.3} {:.3} {:.3}", body[0], body[1], body[2]);
    let Some(pose) = pose else {
        return place;
    };
    let offset = CameraBodyOffset::from_pose_and_body(pose, body);
    format!(
        "{place}; fixed camera offset from it eye x {:.3} z {:.3} look x {:.3} z {:.3} ({})",
        offset.eye_offset_xz[0],
        offset.eye_offset_xz[1],
        offset.look_offset_xz[0],
        offset.look_offset_xz[1],
        if following {
            "follows the body"
        } else {
            "a fixed pose"
        }
    )
}
