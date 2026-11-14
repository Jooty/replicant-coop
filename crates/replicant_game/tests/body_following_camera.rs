use replicant_game::fight_sync::{
    CAMERA_AIM_SENT_AGAIN_AFTER_MS, CameraBodyOffset, FIXED_CAMERA_INDEX, FixedCameraPose,
    GuestAimStep, GuestCameraAimFollow, HostCameraAim, HostCameraAimResend, camera_follows_body,
};

const SWITCH_POSE: FixedCameraPose = FixedCameraPose {
    eye: [-25.0, 85.0, 0.0],
    look_at: [-4.0, 69.0, 0.0],
};

const HOST_BODY_AT_THE_SWITCH: [f32; 3] = [-10.4, 69.7, 4.9];

const FOLLOWED_POSE: FixedCameraPose = FixedCameraPose {
    eye: [-29.279, 85.0, -2.187],
    look_at: [-8.279, 69.0, -2.187],
};

const HOST_BODY_LATER: [f32; 3] = [-13.0, 69.6, -4.8];

const FIGHT_START_POSE: FixedCameraPose = FixedCameraPose {
    eye: [-11.507, 75.47, 0.0],
    look_at: [61.468, 53.076, 0.0],
};

const GUEST_BODY: [f32; 3] = [6.0, 69.6, 20.0];

fn host_after_a_switch() -> HostCameraAimResend {
    let mut host = HostCameraAimResend::default();
    host.sent_with_a_switch(SWITCH_POSE, Some(HOST_BODY_AT_THE_SWITCH), 1_000);
    host
}

fn host_update(
    host: &mut HostCameraAimResend,
    pose: FixedCameraPose,
    body: [f32; 3],
) -> HostCameraAim {
    host.update_for(
        true,
        Some(FIXED_CAMERA_INDEX),
        Some(pose),
        Some(body),
        1_000 + CAMERA_AIM_SENT_AGAIN_AFTER_MS,
    )
    .expect("the moved pose is sent")
}

fn guest_taking(aim: HostCameraAim) -> GuestCameraAimFollow {
    let mut guest = GuestCameraAimFollow::default();
    guest.receive(aim, true);
    guest
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn a_following_camera_is_rebuilt_from_the_guests_own_body() {
    let mut host = host_after_a_switch();
    let aim = host_update(&mut host, FOLLOWED_POSE, HOST_BODY_LATER);
    assert!(host.follows_the_body());
    let offset = aim.follows_body.expect("the camera moved with the body");

    let mut guest = guest_taking(aim);
    assert_eq!(
        guest.tick(true, Some(FIXED_CAMERA_INDEX), Some(GUEST_BODY)),
        None
    );
    let Some(Ok(GuestAimStep::AroundYourBody { pose, first, .. })) =
        guest.tick(true, Some(FIXED_CAMERA_INDEX), Some(GUEST_BODY))
    else {
        panic!("a following pose is built around the guest's body");
    };
    assert!(first);
    assert_eq!(pose, offset.pose_around(GUEST_BODY));
    assert_ne!(pose, FOLLOWED_POSE, "never the host's absolute pose");
    assert!(guest.follows_your_body());

    assert_eq!(
        guest.tick(true, Some(FIXED_CAMERA_INDEX), Some(GUEST_BODY)),
        None,
        "a still body sets nothing again"
    );
    let walked = [GUEST_BODY[0] + 2.0, GUEST_BODY[1], GUEST_BODY[2] - 1.0];
    let Some(Ok(GuestAimStep::AroundYourBody { pose: moved, .. })) =
        guest.tick(true, Some(FIXED_CAMERA_INDEX), Some(walked))
    else {
        panic!("the camera goes where the guest's body goes");
    };
    assert!(near(moved.eye[0] - pose.eye[0], 2.0));
    assert!(near(moved.eye[2] - pose.eye[2], -1.0));

    guest.host_switched();
    assert_eq!(
        guest.tick(true, Some(FIXED_CAMERA_INDEX), Some(GUEST_BODY)),
        None,
        "a host switch ends the following"
    );
}

#[test]
fn a_fixed_pose_is_still_taken_as_it_is() {
    let mut host = HostCameraAimResend::default();
    host.sent_with_a_switch(FIGHT_START_POSE, Some(HOST_BODY_AT_THE_SWITCH), 1_000);
    let jumped = host_update(&mut host, SWITCH_POSE, HOST_BODY_AT_THE_SWITCH);
    assert_eq!(jumped.follows_body, None, "a pose that jumped is fixed");
    assert!(!host.follows_the_body());

    let mut guest = guest_taking(jumped);
    guest.tick(true, Some(FIXED_CAMERA_INDEX), Some(GUEST_BODY));
    assert_eq!(
        guest.tick(true, Some(FIXED_CAMERA_INDEX), Some(GUEST_BODY)),
        Some(Ok(GuestAimStep::HostsPose(SWITCH_POSE)))
    );
    assert_eq!(
        guest.tick(true, Some(FIXED_CAMERA_INDEX), Some(GUEST_BODY)),
        None,
        "a fixed pose is set once"
    );
    assert!(!guest.follows_your_body());

    let new_view = FixedCameraPose {
        look_at: [SWITCH_POSE.look_at[0], 60.0, SWITCH_POSE.look_at[2]],
        ..SWITCH_POSE
    };
    assert!(!camera_follows_body(
        (SWITCH_POSE, HOST_BODY_AT_THE_SWITCH),
        (new_view, HOST_BODY_LATER),
        true
    ));
}

#[test]
fn the_offset_survives_both_players_standing_apart() {
    let mut host = host_after_a_switch();
    let aim = host_update(&mut host, FOLLOWED_POSE, HOST_BODY_LATER);
    let offset = aim.follows_body.expect("following");
    assert_eq!(
        offset,
        CameraBodyOffset::from_pose_and_body(FOLLOWED_POSE, HOST_BODY_LATER)
    );

    let mut guest = guest_taking(aim);
    guest.tick(true, Some(FIXED_CAMERA_INDEX), Some(GUEST_BODY));
    let Some(Ok(GuestAimStep::AroundYourBody { pose, .. })) =
        guest.tick(true, Some(FIXED_CAMERA_INDEX), Some(GUEST_BODY))
    else {
        panic!("following");
    };
    let host_offset = CameraBodyOffset::from_pose_and_body(FOLLOWED_POSE, HOST_BODY_LATER);
    let guest_offset = CameraBodyOffset::from_pose_and_body(pose, GUEST_BODY);
    for axis in 0..2 {
        assert!(near(
            host_offset.eye_offset_xz[axis],
            guest_offset.eye_offset_xz[axis]
        ));
        assert!(near(
            host_offset.look_offset_xz[axis],
            guest_offset.look_offset_xz[axis]
        ));
    }
    assert!(
        near(pose.eye[1], FOLLOWED_POSE.eye[1]),
        "the height is the host's"
    );
    assert!(near(pose.look_at[1], FOLLOWED_POSE.look_at[1]));
    assert!(
        (pose.eye[0] - FOLLOWED_POSE.eye[0]).abs() > 10.0,
        "players apart see different absolute poses"
    );
}
