use crate::fight_sync::{FIXED_CAMERA_INDEX, SNOW_CAMERA_INDEX};

pub const CAMERA_SWITCH_TO_RVA: u32 = 0x34_3330;
pub const HOST_CAMERA_APPLIED_AFTER_TICKS: u32 = 1;

/// The fixed camera's eye and the point it looks at, in world units, as `_SetFixedCamera`
/// 0x6F93E0 takes them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FixedCameraPose {
    pub eye: [f32; 3],
    pub look_at: [f32; 3],
}

/// One switch of the active camera that `CameraManager::SwitchTo` 0x343330 made.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraSwitch {
    pub from: i32,
    pub to: i32,
    pub blend_seconds: f32,
    pub blend_kind: i32,
}

/// Who asked for a switch, as the switch log line names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraSwitchAsker {
    Native,
    HostsMessage,
    GameCode,
}

/// A host camera switch as the guest receives it. `seq` orders the host's switches;
/// `natives_before` is how many `_ChangeCamera` calls the host had run in the fight window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostCamera {
    pub seq: u32,
    pub natives_before: u32,
    pub camera: i32,
    pub blend_seconds: f32,
    pub blend_kind: i32,
    pub pose: Option<FixedCameraPose>,
}

/// What the guest does with a host camera it received.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostCameraReceipt {
    Kept,
    OlderThanTheLast,
    OutsideAFightWindow,
}

/// Why a kept host camera was not applied when its tick came.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostCameraSkip {
    AlreadyActive,
    OlderThanANative,
    NoChangeKind,
}

/// The guest's side: keeps the newest host camera and applies it one tick after it came, only
/// in a fight window and only when its own active camera still differs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GuestCameraFollow {
    pending: Option<(HostCamera, u32)>,
    last_seq: Option<u32>,
    natives_in_window: u32,
}

/// The host's side: numbers its switches and counts its `_ChangeCamera` calls in a window.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostCameraFollow {
    next_seq: u32,
    natives_in_window: u32,
}

impl CameraSwitchAsker {
    pub fn text(self) -> &'static str {
        match self {
            Self::Native => "a native",
            Self::HostsMessage => "the host's message",
            Self::GameCode => "game code",
        }
    }
}

impl HostCameraFollow {
    pub const fn new() -> Self {
        Self {
            next_seq: 0,
            natives_in_window: 0,
        }
    }

    pub fn note_native(&mut self, in_window: bool) {
        if in_window {
            self.natives_in_window += 1;
        }
    }

    /// The message for a switch the host made, or `None` outside a fight window.
    pub fn message_for(
        &mut self,
        switch: CameraSwitch,
        pose: Option<FixedCameraPose>,
        in_window: bool,
    ) -> Option<HostCamera> {
        if !in_window {
            return None;
        }
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        Some(HostCamera {
            seq,
            natives_before: self.natives_in_window,
            camera: switch.to,
            blend_seconds: switch.blend_seconds,
            blend_kind: switch.blend_kind,
            pose: if switch.to == FIXED_CAMERA_INDEX {
                pose
            } else {
                None
            },
        })
    }

    pub fn window_closed(&mut self) {
        self.natives_in_window = 0;
    }
}

impl GuestCameraFollow {
    pub const fn new() -> Self {
        Self {
            pending: None,
            last_seq: None,
            natives_in_window: 0,
        }
    }

    pub fn note_native(&mut self, in_window: bool) {
        if in_window {
            self.natives_in_window += 1;
        }
    }

    pub fn receive(&mut self, camera: HostCamera, in_window: bool) -> HostCameraReceipt {
        if !in_window {
            return HostCameraReceipt::OutsideAFightWindow;
        }
        if self.last_seq.is_some_and(|last| camera.seq <= last) {
            return HostCameraReceipt::OlderThanTheLast;
        }
        self.last_seq = Some(camera.seq);
        self.pending = Some((camera, 0));
        HostCameraReceipt::Kept
    }

    /// Called once a tick: the host camera to apply now, or why the kept one was dropped.
    pub fn tick(
        &mut self,
        in_window: bool,
        own_camera: Option<i32>,
    ) -> Option<Result<HostCamera, (HostCamera, HostCameraSkip)>> {
        if !in_window {
            self.window_closed();
            return None;
        }
        let (camera, ticks) = self.pending.as_mut()?;
        if *ticks < HOST_CAMERA_APPLIED_AFTER_TICKS {
            *ticks += 1;
            return None;
        }
        let camera = *camera;
        self.pending = None;
        let skip = if camera.natives_before < self.natives_in_window {
            Some(HostCameraSkip::OlderThanANative)
        } else if own_camera == Some(camera.camera) {
            Some(HostCameraSkip::AlreadyActive)
        } else if change_kind_of_camera(camera.camera).is_none() {
            Some(HostCameraSkip::NoChangeKind)
        } else {
            None
        };
        Some(match skip {
            Some(skip) => Err((camera, skip)),
            None => Ok(camera),
        })
    }

    pub fn window_closed(&mut self) {
        self.pending = None;
        self.last_seq = None;
        self.natives_in_window = 0;
    }
}

/// The `_ChangeCamera` kind that makes camera `index` active, where one does.
pub fn change_kind_of_camera(index: i32) -> Option<i32> {
    match index {
        SNOW_CAMERA_INDEX => Some(0),
        FIXED_CAMERA_INDEX => Some(1),
        _ => None,
    }
}

pub fn camera_switch_line(switch: CameraSwitch, asker: CameraSwitchAsker) -> String {
    format!(
        "camera switch: camera {} to camera {} (blend {:.2} s, kind {}), asked by {}",
        switch.from,
        switch.to,
        switch.blend_seconds,
        switch.blend_kind,
        asker.text()
    )
}
