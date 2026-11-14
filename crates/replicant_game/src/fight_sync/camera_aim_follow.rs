use crate::fight_sync::FIXED_CAMERA_INDEX;
use crate::fight_sync::body_following_camera::{CameraBodyOffset, camera_follows_body};
use crate::fight_sync::camera_follow::{FixedCameraPose, HOST_CAMERA_APPLIED_AFTER_TICKS};

pub const CAMERA_AIM_MOVE_SENT_AGAIN_WORLD_UNITS: f32 = 0.05;
pub const CAMERA_AIM_SENT_AGAIN_AFTER_MS: u64 = 100;

/// A pose-only update of the host's fixed camera while it stays active: no switch comes with
/// it. A higher `seq` replaces a lower one. `follows_body` is set when the camera follows the
/// host's body: the guest then builds the pose around its own body instead of taking `pose`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostCameraAim {
    pub seq: u32,
    pub pose: FixedCameraPose,
    pub follows_body: Option<CameraBodyOffset>,
}

/// A pose the guest sets on its fixed camera now: the host's as it is, or the host's camera
/// rebuilt around the guest's body (`first` for the first one of a received update).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GuestAimStep {
    HostsPose(FixedCameraPose),
    AroundYourBody {
        pose: FixedCameraPose,
        offset: CameraBodyOffset,
        first: bool,
    },
}

/// What the guest does with a pose update it received.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraAimReceipt {
    Kept,
    OlderThanTheLast,
    OutsideAFightWindow,
}

/// The host's side: sends the fixed camera's pose again when it moved since the last one sent
/// (with a switch or alone), at most once per `CAMERA_AIM_SENT_AGAIN_AFTER_MS`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HostCameraAimResend {
    next_seq: u32,
    last_sent: Option<(FixedCameraPose, u64)>,
    last_body: Option<[f32; 3]>,
    following: bool,
}

/// The guest's side: keeps the newest pose update and applies it one tick later, only while
/// its own active camera is the fixed one; a following one is rebuilt around the guest's body
/// every tick until another update or a switch. It never switches cameras.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GuestCameraAimFollow {
    pending: Option<(HostCameraAim, u32)>,
    last_seq: Option<u32>,
    following: Option<CameraBodyOffset>,
    last_applied: Option<FixedCameraPose>,
}

impl HostCameraAimResend {
    pub const fn new() -> Self {
        Self {
            next_seq: 0,
            last_sent: None,
            last_body: None,
            following: false,
        }
    }

    /// A switch message carried `pose` at `now_ms`, the host's body at `body`: later updates are
    /// measured from it, and the camera counts as fixed until it moves with the body.
    pub fn sent_with_a_switch(
        &mut self,
        pose: FixedCameraPose,
        body: Option<[f32; 3]>,
        now_ms: u64,
    ) {
        self.last_sent = Some((pose, now_ms));
        self.last_body = body;
        self.following = false;
    }

    /// The pose update to send this tick, if the fixed camera is active and its pose moved;
    /// `body` is the host's own body now.
    pub fn update_for(
        &mut self,
        in_window: bool,
        active_camera: Option<i32>,
        pose: Option<FixedCameraPose>,
        body: Option<[f32; 3]>,
        now_ms: u64,
    ) -> Option<HostCameraAim> {
        if !in_window {
            self.window_closed();
            return None;
        }
        let pose = pose?;
        if active_camera != Some(FIXED_CAMERA_INDEX) {
            return None;
        }
        if let Some((last, at_ms)) = self.last_sent {
            if !pose_moved(last, pose) {
                return None;
            }
            if now_ms.saturating_sub(at_ms) < CAMERA_AIM_SENT_AGAIN_AFTER_MS {
                return None;
            }
        }
        self.following = match (self.last_sent, self.last_body, body) {
            (Some((last, _)), Some(last_body), Some(body)) => {
                camera_follows_body((last, last_body), (pose, body), self.following)
            }
            _ => false,
        };
        self.last_sent = Some((pose, now_ms));
        self.last_body = body;
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        let follows_body = body
            .filter(|_| self.following)
            .map(|body| CameraBodyOffset::from_pose_and_body(pose, body));
        Some(HostCameraAim {
            seq,
            pose,
            follows_body,
        })
    }

    pub fn follows_the_body(&self) -> bool {
        self.following
    }

    pub fn window_closed(&mut self) {
        self.last_sent = None;
        self.last_body = None;
        self.following = false;
    }
}

impl GuestCameraAimFollow {
    pub const fn new() -> Self {
        Self {
            pending: None,
            last_seq: None,
            following: None,
            last_applied: None,
        }
    }

    pub fn follows_your_body(&self) -> bool {
        self.following.is_some()
    }

    /// The host switched cameras: a body-following pose no longer applies.
    pub fn host_switched(&mut self) {
        self.following = None;
        self.last_applied = None;
    }

    pub fn receive(&mut self, aim: HostCameraAim, in_window: bool) -> CameraAimReceipt {
        if !in_window {
            return CameraAimReceipt::OutsideAFightWindow;
        }
        if self.last_seq.is_some_and(|last| aim.seq <= last) {
            return CameraAimReceipt::OlderThanTheLast;
        }
        self.last_seq = Some(aim.seq);
        self.pending = Some((aim, 0));
        CameraAimReceipt::Kept
    }

    /// Called once a tick with the guest's own body: `Ok` is a pose to set on the fixed camera
    /// now (no switch), `Err` a kept update dropped because the guest's own active camera is not
    /// the fixed one.
    pub fn tick(
        &mut self,
        in_window: bool,
        own_camera: Option<i32>,
        own_body: Option<[f32; 3]>,
    ) -> Option<Result<GuestAimStep, HostCameraAim>> {
        if !in_window {
            self.window_closed();
            return None;
        }
        let mut first = false;
        if let Some((aim, ticks)) = self.pending.as_mut() {
            if *ticks < HOST_CAMERA_APPLIED_AFTER_TICKS {
                *ticks += 1;
            } else {
                let aim = *aim;
                self.pending = None;
                if own_camera != Some(FIXED_CAMERA_INDEX) {
                    return Some(Err(aim));
                }
                self.following = aim.follows_body;
                self.last_applied = None;
                if aim.follows_body.is_none() {
                    return Some(Ok(GuestAimStep::HostsPose(aim.pose)));
                }
                first = true;
            }
        }
        let offset = self.following?;
        if own_camera != Some(FIXED_CAMERA_INDEX) {
            return None;
        }
        let pose = offset.pose_around(own_body?);
        if !first
            && self
                .last_applied
                .is_some_and(|last| !pose_moved(last, pose))
        {
            return None;
        }
        self.last_applied = Some(pose);
        Some(Ok(GuestAimStep::AroundYourBody {
            pose,
            offset,
            first,
        }))
    }

    pub fn window_closed(&mut self) {
        self.pending = None;
        self.last_seq = None;
        self.following = None;
        self.last_applied = None;
    }
}

pub fn pose_moved(from: FixedCameraPose, to: FixedCameraPose) -> bool {
    from.eye
        .iter()
        .chain(&from.look_at)
        .zip(to.eye.iter().chain(&to.look_at))
        .any(|(a, b)| (a - b).abs() > CAMERA_AIM_MOVE_SENT_AGAIN_WORLD_UNITS)
}
