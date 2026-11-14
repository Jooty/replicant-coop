use core::ffi::c_void;
use core::ptr;
use std::panic;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::{Mutex, PoisonError};

use replicant_game::fight_sync::{
    CAMERA_SWITCH_TO_RVA, CameraAimReceipt, CameraBodyOffset, CameraSwitch, CameraSwitchAsker,
    FixedCameraPose, GuestAimStep, GuestCameraAimFollow, GuestCameraFollow, HostCamera,
    HostCameraAim, HostCameraAimResend, HostCameraFollow, HostCameraReceipt, HostCameraSkip,
    SET_FIXED_CAMERA_NATIVE_RVA, camera_switch_line, change_kind_of_camera, read_fixed_camera_pose,
};
use replicant_game::marker::CAMERA_ACTIVE_INDEX_RVA;
use replicant_game::side_view::CHANGE_CAMERA_NATIVE_RVA;
use replicant_game::stay_together::FollowRole;
use replicant_game::{GameMemory, Replicant};
use replicant_proto::{
    FightCamera, FightCameraAim, FightCameraBodyOffset, FightCameraPose, Message,
};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::fight_sync::state::{current_phase_index, here};
use crate::game_hooks::hook_address;
use crate::memory::InProcessMemory;
use crate::net;
use crate::net::net_clock_ms;
use crate::story_jump::{NativeArg, call_native_with};

const MAX_SWITCHES_KEPT_BETWEEN_TICKS: usize = 64;

static SWITCH_TO_ORIGINAL: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static INSIDE_A_CAMERA_NATIVE: AtomicBool = AtomicBool::new(false);
static APPLYING_HOSTS_CAMERA: AtomicBool = AtomicBool::new(false);
static APPLYING_HOSTS_AIM: AtomicBool = AtomicBool::new(false);
static SWITCHES: Mutex<Vec<(CameraSwitch, CameraSwitchAsker)>> = Mutex::new(Vec::new());
static HOST_FOLLOW: Mutex<HostCameraFollow> = Mutex::new(HostCameraFollow::new());
static GUEST_FOLLOW: Mutex<GuestCameraFollow> = Mutex::new(GuestCameraFollow::new());
static HOST_AIM: Mutex<HostCameraAimResend> = Mutex::new(HostCameraAimResend::new());
static GUEST_AIM: Mutex<GuestCameraAimFollow> = Mutex::new(GuestCameraAimFollow::new());

type SwitchTo = unsafe extern "system" fn(*mut c_void, i32, f32, i32, u8, *const c_void);

pub(super) fn install(build: VerifiedBuild) {
    #[expect(unsafe_code, reason = "hooks CameraManager::SwitchTo by its address")]
    // SAFETY: the RVA starts `CameraManager::SwitchTo` in the verified build (symbols.tsv):
    // manager, index, blend seconds (xmm2), blend kind, a byte and a pointer; the detour matches.
    unsafe {
        hook_address(
            "CameraManager::SwitchTo",
            (build.module_base() + u64::from(CAMERA_SWITCH_TO_RVA)) as usize,
            switch_to_detour as *const () as *mut c_void,
            &SWITCH_TO_ORIGINAL,
        );
    }
}

pub(super) fn camera_native_running(running: bool) {
    INSIDE_A_CAMERA_NATIVE.store(running, Ordering::Release);
}

pub(super) fn mod_is_setting_the_camera() -> bool {
    APPLYING_HOSTS_AIM.load(Ordering::Acquire) || APPLYING_HOSTS_CAMERA.load(Ordering::Acquire)
}

pub(super) fn applying_hosts_aim() -> bool {
    APPLYING_HOSTS_AIM.load(Ordering::Acquire)
}

pub(super) fn camera_follows_a_body(is_guest: bool) -> bool {
    if is_guest {
        GUEST_AIM
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .follows_your_body()
    } else {
        HOST_AIM
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .follows_the_body()
    }
}

pub(super) fn own_body(build: &VerifiedBuild) -> Option<[f32; 3]> {
    let position = Replicant::new(InProcessMemory::new(build))
        .position()
        .ok()?;
    let body = [position.x, position.y, position.z];
    body.iter().all(|axis| axis.is_finite()).then_some(body)
}

pub(super) fn note_change_camera_native(in_window: bool) {
    if APPLYING_HOSTS_CAMERA.load(Ordering::Acquire) {
        return;
    }
    HOST_FOLLOW
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .note_native(in_window);
    GUEST_FOLLOW
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .note_native(in_window);
}

pub(super) fn tick(build: &VerifiedBuild) {
    let switches = core::mem::take(&mut *SWITCHES.lock().unwrap_or_else(PoisonError::into_inner));
    let Some(net) = net::handle() else {
        return;
    };
    let received = net.take_relayed(|message| {
        matches!(
            message,
            Message::FightCamera(_) | Message::FightCameraAim(_)
        )
    });
    let phase = current_phase_index(build);
    let in_window = phase.is_some_and(crate::fight_sync::covers_phase);
    if in_window {
        for (switch, asker) in &switches {
            log_line(&camera_switch_line(*switch, *asker));
        }
    }
    match here(build).map(|here| here.role) {
        Some(FollowRole::Host) => host_tick(build, phase, in_window, &switches),
        Some(FollowRole::Guest) => guest_tick(build, phase, in_window, received),
        Some(_) | None => {}
    }
}

fn host_tick(
    build: &VerifiedBuild,
    phase: Option<u16>,
    in_window: bool,
    switches: &[(CameraSwitch, CameraSwitchAsker)],
) {
    let mut follow = HOST_FOLLOW.lock().unwrap_or_else(PoisonError::into_inner);
    let mut aim = HOST_AIM.lock().unwrap_or_else(PoisonError::into_inner);
    let (Some(phase), true) = (phase, in_window) else {
        follow.window_closed();
        aim.window_closed();
        return;
    };
    let now_ms = net_clock_ms();
    let pose = read_fixed_camera_pose(&InProcessMemory::new(build));
    let body = own_body(build);
    let messages: Vec<HostCamera> = switches
        .iter()
        .filter_map(|(switch, _)| follow.message_for(*switch, pose, true))
        .collect();
    for sent_pose in messages.iter().filter_map(|camera| camera.pose) {
        aim.sent_with_a_switch(sent_pose, body, now_ms);
    }
    let update = aim.update_for(true, active_camera(build), pose, body, now_ms);
    drop(aim);
    drop(follow);
    let Some(net) = net::handle() else {
        return;
    };
    for camera in messages {
        log_line(&format!(
            "camera follow: sent your camera {} (switch {}, after {} change-camera natives{})",
            camera.camera,
            camera.seq,
            camera.natives_before,
            pose_text(camera.pose)
        ));
        net.send_relayed(Message::FightCamera(to_wire(phase, camera)));
    }
    if let Some(update) = update {
        log_line(&format!(
            "camera follow: sent your fixed camera's pose again (update {}{}, {})",
            update.seq,
            pose_text(Some(update.pose)),
            update
                .follows_body
                .map_or_else(|| "a fixed pose".to_owned(), offset_text)
        ));
        net.send_relayed(Message::FightCameraAim(aim_to_wire(phase, update)));
    }
}

fn guest_tick(build: &VerifiedBuild, phase: Option<u16>, in_window: bool, received: Vec<Message>) {
    let mut follow = GUEST_FOLLOW.lock().unwrap_or_else(PoisonError::into_inner);
    let mut aims = GUEST_AIM.lock().unwrap_or_else(PoisonError::into_inner);
    for message in received {
        let in_this_window = |message_phase: u16| in_window && phase == Some(message_phase);
        match message {
            Message::FightCamera(wire) => {
                let receipt = follow.receive(from_wire(wire), in_this_window(wire.phase));
                if receipt == HostCameraReceipt::Kept {
                    aims.host_switched();
                } else {
                    log_line(&format!(
                        "camera follow: the host's camera {} (switch {}) not kept: {receipt:?}",
                        wire.camera, wire.seq
                    ));
                }
            }
            Message::FightCameraAim(wire) => {
                let receipt = aims.receive(aim_from_wire(wire), in_this_window(wire.phase));
                if receipt != CameraAimReceipt::Kept {
                    log_line(&format!(
                        "camera follow: the host's fixed camera pose (update {}) not kept: {receipt:?}",
                        wire.seq
                    ));
                }
            }
            _ => {}
        }
    }
    let step = follow.tick(in_window, active_camera(build));
    drop(follow);
    match step {
        Some(Ok(camera)) => apply_hosts_camera(build, camera),
        Some(Err((camera, skip))) => log_skip(camera, skip),
        None => {}
    }
    let aim_step = aims.tick(in_window, active_camera(build), own_body(build));
    drop(aims);
    match aim_step {
        Some(Ok(GuestAimStep::HostsPose(pose))) => {
            log_line(&format!(
                "camera follow: setting the host's fixed camera pose{}",
                pose_text(Some(pose))
            ));
            apply_hosts_aim(build, pose);
        }
        Some(Ok(GuestAimStep::AroundYourBody {
            pose,
            offset,
            first,
        })) => {
            if first {
                log_line(&format!(
                    "camera follow: the host's fixed camera follows its body; building it around your body ({}{})",
                    offset_text(offset),
                    pose_text(Some(pose))
                ));
            }
            apply_hosts_aim(build, pose);
        }
        Some(Err(aim)) => log_line(&format!(
            "camera follow: the host's fixed camera pose (update {}) not taken: your active camera is not the fixed one",
            aim.seq
        )),
        None => {}
    }
}

fn active_camera(build: &VerifiedBuild) -> Option<i32> {
    InProcessMemory::new(build)
        .read_i32(build.module_base() + u64::from(CAMERA_ACTIVE_INDEX_RVA))
        .ok()
}

fn log_skip(camera: HostCamera, skip: HostCameraSkip) {
    let why = match skip {
        HostCameraSkip::AlreadyActive => "your game already has it",
        HostCameraSkip::OlderThanANative => "a newer change-camera native ran here since",
        HostCameraSkip::NoChangeKind => "no change-camera kind makes it active",
    };
    log_line(&format!(
        "camera follow: the host's camera {} (switch {}) not taken: {why}",
        camera.camera, camera.seq
    ));
}

fn apply_hosts_camera(build: &VerifiedBuild, camera: HostCamera) {
    let Some(kind) = change_kind_of_camera(camera.camera) else {
        return;
    };
    log_line(&format!(
        "camera follow: taking the host's camera {} (switch {}, blend {:.2} s, kind {}{})",
        camera.camera,
        camera.seq,
        camera.blend_seconds,
        camera.blend_kind,
        pose_text(camera.pose)
    ));
    APPLYING_HOSTS_CAMERA.store(true, Ordering::Release);
    if let Some(pose) = camera.pose {
        let reals: Vec<NativeArg> = pose
            .eye
            .iter()
            .chain(&pose.look_at)
            .map(|axis| NativeArg::Real(f64::from(*axis)))
            .collect();
        call_native_with(build, SET_FIXED_CAMERA_NATIVE_RVA, &reals);
    }
    call_native_with(
        build,
        CHANGE_CAMERA_NATIVE_RVA,
        &[
            NativeArg::Integer(kind),
            NativeArg::Real(f64::from(camera.blend_seconds)),
            NativeArg::Integer(camera.blend_kind),
        ],
    );
    APPLYING_HOSTS_CAMERA.store(false, Ordering::Release);
}

fn apply_hosts_aim(build: &VerifiedBuild, pose: FixedCameraPose) {
    let reals: Vec<NativeArg> = pose
        .eye
        .iter()
        .chain(&pose.look_at)
        .map(|axis| NativeArg::Real(f64::from(*axis)))
        .collect();
    APPLYING_HOSTS_AIM.store(true, Ordering::Release);
    call_native_with(build, SET_FIXED_CAMERA_NATIVE_RVA, &reals);
    APPLYING_HOSTS_AIM.store(false, Ordering::Release);
}

fn pose_text(pose: Option<FixedCameraPose>) -> String {
    pose.map_or_else(String::new, |pose| {
        format!(
            ", eye {:.3} {:.3} {:.3} look {:.3} {:.3} {:.3}",
            pose.eye[0],
            pose.eye[1],
            pose.eye[2],
            pose.look_at[0],
            pose.look_at[1],
            pose.look_at[2]
        )
    })
}

fn offset_text(offset: CameraBodyOffset) -> String {
    format!(
        "follows the body, offset eye x {:.3} z {:.3} height {:.3} look x {:.3} z {:.3} height {:.3}",
        offset.eye_offset_xz[0],
        offset.eye_offset_xz[1],
        offset.eye_height,
        offset.look_offset_xz[0],
        offset.look_offset_xz[1],
        offset.look_height
    )
}

fn to_wire(phase: u16, camera: HostCamera) -> FightCamera {
    FightCamera {
        phase,
        seq: camera.seq,
        natives_before: camera.natives_before,
        camera: camera.camera,
        blend_seconds: camera.blend_seconds,
        blend_kind: camera.blend_kind,
        pose: camera.pose.map(|pose| FightCameraPose {
            eye: pose.eye,
            look_at: pose.look_at,
        }),
    }
}

fn aim_to_wire(phase: u16, aim: HostCameraAim) -> FightCameraAim {
    FightCameraAim {
        phase,
        seq: aim.seq,
        pose: FightCameraPose {
            eye: aim.pose.eye,
            look_at: aim.pose.look_at,
        },
        follows_body: aim.follows_body.map(|offset| FightCameraBodyOffset {
            eye_offset_xz: offset.eye_offset_xz,
            eye_height: offset.eye_height,
            look_offset_xz: offset.look_offset_xz,
            look_height: offset.look_height,
        }),
    }
}

fn aim_from_wire(wire: FightCameraAim) -> HostCameraAim {
    HostCameraAim {
        seq: wire.seq,
        pose: FixedCameraPose {
            eye: wire.pose.eye,
            look_at: wire.pose.look_at,
        },
        follows_body: wire.follows_body.map(|offset| CameraBodyOffset {
            eye_offset_xz: offset.eye_offset_xz,
            eye_height: offset.eye_height,
            look_offset_xz: offset.look_offset_xz,
            look_height: offset.look_height,
        }),
    }
}

fn from_wire(wire: FightCamera) -> HostCamera {
    HostCamera {
        seq: wire.seq,
        natives_before: wire.natives_before,
        camera: wire.camera,
        blend_seconds: wire.blend_seconds,
        blend_kind: wire.blend_kind,
        pose: wire.pose.map(|pose| FixedCameraPose {
            eye: pose.eye,
            look_at: pose.look_at,
        }),
    }
}

fn asker_now() -> CameraSwitchAsker {
    if APPLYING_HOSTS_CAMERA.load(Ordering::Acquire) {
        CameraSwitchAsker::HostsMessage
    } else if INSIDE_A_CAMERA_NATIVE.load(Ordering::Acquire) {
        CameraSwitchAsker::Native
    } else {
        CameraSwitchAsker::GameCode
    }
}

#[expect(
    unsafe_code,
    reason = "reads the manager's active index and calls the original through the trampoline"
)]
extern "system" fn switch_to_detour(
    manager: *mut c_void,
    index: i32,
    blend_seconds: f32,
    blend_kind: i32,
    flag: u8,
    lerp: *const c_void,
) {
    let trampoline = SWITCH_TO_ORIGINAL.load(Ordering::Acquire);
    if trampoline.is_null() {
        return;
    }
    // SAFETY: the game passes its camera manager, whose first int is the active index.
    let from = unsafe { manager.cast::<i32>().read_volatile() };
    // SAFETY: the trampoline continues into the original `SwitchTo`, which has this signature.
    unsafe {
        core::mem::transmute::<*mut c_void, SwitchTo>(trampoline)(
            manager,
            index,
            blend_seconds,
            blend_kind,
            flag,
            lerp,
        );
    }
    // SAFETY: as above; the manager outlives the call.
    let to = unsafe { manager.cast::<i32>().read_volatile() };
    if from == to {
        return;
    }
    let _ = panic::catch_unwind(|| {
        let switch = CameraSwitch {
            from,
            to,
            blend_seconds,
            blend_kind,
        };
        let mut switches = SWITCHES.lock().unwrap_or_else(PoisonError::into_inner);
        if switches.len() < MAX_SWITCHES_KEPT_BETWEEN_TICKS {
            switches.push((switch, asker_now()));
        }
    });
}
