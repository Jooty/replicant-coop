use replicant_game::fight_sync::{
    CAMERA_AIM_SENT_AGAIN_AFTER_MS, CameraAimReceipt, FIXED_CAMERA_INDEX, FixedCameraPose,
    GuestAimStep, GuestCameraAimFollow, GuestCameraFollow, HostCamera, HostCameraAim,
    HostCameraAimResend, SNOW_CAMERA_INDEX,
};

const SWITCH_POSE: FixedCameraPose = FixedCameraPose {
    eye: [-25.0, 85.0, 0.0],
    look_at: [-14.165, 74.804, 6.049],
};

const SETTLED_POSE: FixedCameraPose = FixedCameraPose {
    eye: [-25.0, 85.0, 0.0],
    look_at: [-12.23, 75.27, 0.0],
};

const TICK_MS: u64 = 16;

fn aim(seq: u32) -> HostCameraAim {
    HostCameraAim {
        seq,
        pose: SETTLED_POSE,
        follows_body: None,
    }
}

fn nudged(pose: FixedCameraPose, by: f32) -> FixedCameraPose {
    FixedCameraPose {
        look_at: [pose.look_at[0] + by, pose.look_at[1], pose.look_at[2]],
        ..pose
    }
}

#[test]
fn a_pose_is_sent_again_when_it_moved() {
    let mut host = HostCameraAimResend::default();
    host.sent_with_a_switch(SWITCH_POSE, None, 1_000);
    let update = host.update_for(
        true,
        Some(FIXED_CAMERA_INDEX),
        Some(SETTLED_POSE),
        None,
        1_000 + CAMERA_AIM_SENT_AGAIN_AFTER_MS,
    );
    assert_eq!(
        update,
        Some(HostCameraAim {
            seq: 0,
            pose: SETTLED_POSE,
            follows_body: None,
        })
    );
    let next = host.update_for(
        true,
        Some(FIXED_CAMERA_INDEX),
        Some(nudged(SETTLED_POSE, 1.0)),
        None,
        1_000 + 2 * CAMERA_AIM_SENT_AGAIN_AFTER_MS,
    );
    assert_eq!(next.map(|aim| aim.seq), Some(1));
}

#[test]
fn a_still_pose_is_not_sent_again() {
    let mut host = HostCameraAimResend::default();
    host.sent_with_a_switch(SETTLED_POSE, None, 0);
    for tick in 1..200 {
        let pose = nudged(SETTLED_POSE, 0.001);
        assert_eq!(
            host.update_for(
                true,
                Some(FIXED_CAMERA_INDEX),
                Some(pose),
                None,
                tick * TICK_MS
            ),
            None
        );
    }
}

#[test]
fn no_more_than_ten_poses_a_second() {
    let mut host = HostCameraAimResend::default();
    host.sent_with_a_switch(SWITCH_POSE, None, 0);
    let mut sent = 0;
    for tick in 1..=(1_000 / TICK_MS) {
        let pose = nudged(SWITCH_POSE, tick as f32);
        if host
            .update_for(
                true,
                Some(FIXED_CAMERA_INDEX),
                Some(pose),
                None,
                tick * TICK_MS,
            )
            .is_some()
        {
            sent += 1;
        }
    }
    assert!((1..=10).contains(&sent), "sent {sent} in a second");
}

#[test]
fn no_pose_is_sent_off_the_fixed_camera_or_outside_a_window() {
    let mut host = HostCameraAimResend::default();
    let far = nudged(SWITCH_POSE, 5.0);
    assert_eq!(
        host.update_for(true, Some(SNOW_CAMERA_INDEX), Some(far), None, 500),
        None
    );
    assert_eq!(
        host.update_for(false, Some(FIXED_CAMERA_INDEX), Some(far), None, 500),
        None
    );
    assert!(
        host.update_for(true, Some(FIXED_CAMERA_INDEX), Some(far), None, 600)
            .is_some()
    );
}

#[test]
fn a_pose_update_is_applied_only_on_the_fixed_camera() {
    let mut guest = GuestCameraAimFollow::default();
    assert_eq!(guest.receive(aim(0), true), CameraAimReceipt::Kept);
    assert_eq!(guest.tick(true, Some(FIXED_CAMERA_INDEX), None), None);
    assert_eq!(
        guest.tick(true, Some(FIXED_CAMERA_INDEX), None),
        Some(Ok(GuestAimStep::HostsPose(SETTLED_POSE)))
    );

    guest.receive(aim(1), true);
    guest.tick(true, Some(SNOW_CAMERA_INDEX), None);
    assert_eq!(
        guest.tick(true, Some(SNOW_CAMERA_INDEX), None),
        Some(Err(aim(1)))
    );

    assert_eq!(
        guest.receive(aim(1), true),
        CameraAimReceipt::OlderThanTheLast
    );
    assert_eq!(
        guest.receive(aim(2), false),
        CameraAimReceipt::OutsideAFightWindow
    );
}

#[test]
fn a_pose_update_never_switches_cameras() {
    let mut switches = GuestCameraFollow::default();
    let mut aims = GuestCameraAimFollow::default();
    aims.receive(aim(0), true);
    for _ in 0..3 {
        assert_eq!(switches.tick(true, Some(SNOW_CAMERA_INDEX)), None);
        let step = aims.tick(true, Some(SNOW_CAMERA_INDEX), None);
        assert!(!matches!(step, Some(Ok(_))), "a pose was set off camera 8");
    }
    let switch_back = HostCamera {
        seq: 0,
        natives_before: 0,
        camera: SNOW_CAMERA_INDEX,
        blend_seconds: 0.0,
        blend_kind: 2,
        pose: None,
    };
    switches.receive(switch_back, true);
    aims.receive(aim(1), true);
    aims.tick(true, Some(FIXED_CAMERA_INDEX), None);
    assert_eq!(
        aims.tick(true, Some(FIXED_CAMERA_INDEX), None),
        Some(Ok(GuestAimStep::HostsPose(SETTLED_POSE)))
    );
    switches.tick(true, Some(FIXED_CAMERA_INDEX));
    assert_eq!(
        switches.tick(true, Some(FIXED_CAMERA_INDEX)),
        Some(Ok(switch_back)),
        "the switch the host asked for is still the switch's to make"
    );
}
