use replicant_proto::{
    MAX_MESSAGE_BYTES, MAX_MOTION_FRAME, MAX_RIDDEN_BOAR_OFFSET_ABS, MAX_YAW_ABS_RADIANS, Message,
    MotionState, PROTOCOL_VERSION, PartnerActivity, PlayerState, RiddenBoar, StateTrack, WireError,
    decode, encode,
};

fn boar() -> RiddenBoar {
    RiddenBoar {
        offset_from_rider: [0.25, -1.5, -0.5],
        yaw_from_rider_radians: 0.1,
        motion: Some(walking(4.0)),
    }
}

fn walking(frame: f32) -> MotionState {
    MotionState {
        index: 7,
        frame,
        speed: 1.0,
        looping: true,
        name_hash: 0,
    }
}

fn rider(tick: u32, boar: Option<RiddenBoar>) -> PlayerState {
    PlayerState {
        tick,
        phase: 30,
        position: [10.0, 2.0, -4.0],
        yaw_radians: 0.5,
        motion: None,
        hp: 10,
        max_hp: 10,
        weapon: None,
        activity: PartnerActivity::Playing,
        mp: 0,
        max_mp: 0,
        level: 1,
        ridden_boar: boar,
    }
}

#[test]
fn the_protocol_number_went_up_with_the_ridden_boar() {
    assert_eq!(PROTOCOL_VERSION, 67);
}

#[test]
fn a_state_with_a_ridden_boar_round_trips() {
    let message = Message::State(rider(1, Some(boar())));
    assert_eq!(message.validate(), Ok(()));
    let bytes = encode(&message).expect("encodes");
    assert!(bytes.len() < MAX_MESSAGE_BYTES / 2);
    assert_eq!(decode(&bytes), Ok(message));
}

#[test]
fn a_state_without_a_boar_says_the_sender_rides_nothing() {
    let message = Message::State(rider(1, None));
    let Ok(Message::State(state)) = decode(&encode(&message).expect("encodes")) else {
        panic!("a state comes back");
    };
    assert_eq!(state.ridden_boar, None);
}

#[test]
fn a_boar_too_far_from_its_rider_or_with_a_bad_yaw_or_motion_is_refused() {
    let mut far = boar();
    far.offset_from_rider[1] = MAX_RIDDEN_BOAR_OFFSET_ABS + 1.0;
    let mut nowhere = boar();
    nowhere.offset_from_rider[0] = f32::NAN;
    let mut spinning = boar();
    spinning.yaw_from_rider_radians = MAX_YAW_ABS_RADIANS + 1.0;
    let mut broken_motion = boar();
    broken_motion.motion = Some(walking(MAX_MOTION_FRAME + 1.0));
    for (bad, field) in [
        (far, "ridden boar offset"),
        (nowhere, "ridden boar offset"),
        (spinning, "ridden boar yaw"),
        (broken_motion, "motion_frame"),
    ] {
        assert_eq!(bad.validate(), Err(WireError::OutOfRange { field }));
        assert_eq!(
            encode(&Message::State(rider(1, Some(bad)))),
            Err(WireError::OutOfRange { field })
        );
    }
}

#[test]
fn the_boar_appears_with_the_first_riding_sample_and_goes_with_the_first_one_without() {
    let mut track = StateTrack::new();
    track.push(0, rider(1, None));
    track.push(100, rider(2, Some(boar())));
    track.push(200, rider(3, Some(boar())));
    track.push(300, rider(4, None));
    let boar_at = |at_ms: u64| track.sample(at_ms, 0).and_then(|sample| sample.ridden_boar);
    assert_eq!(boar_at(50), None);
    assert!(boar_at(100).is_some());
    assert!(boar_at(250).is_some());
    assert_eq!(boar_at(300), None);
    assert_eq!(boar_at(900), None);
}

#[test]
fn the_boar_moves_and_plays_on_smoothly_between_two_samples() {
    let later = RiddenBoar {
        offset_from_rider: [0.25, -0.5, -0.5],
        yaw_from_rider_radians: 0.3,
        motion: Some(walking(10.0)),
    };
    let mut track = StateTrack::new();
    track.push(0, rider(1, Some(boar())));
    track.push(100, rider(2, Some(later)));
    let halfway = track
        .sample(50, 0)
        .and_then(|sample| sample.ridden_boar)
        .expect("a boar halfway");
    assert!((halfway.offset_from_rider[1] + 1.0).abs() < 1e-4);
    assert!((halfway.yaw_from_rider_radians - 0.2).abs() < 1e-4);
    assert!((halfway.motion.expect("motion").frame - 7.0).abs() < 1e-4);
}

#[test]
fn a_late_state_keeps_the_boar_playing_at_its_own_speed() {
    let mut track = StateTrack::new();
    track.push(0, rider(1, Some(boar())));
    let held = track
        .sample(100, 0)
        .and_then(|sample| sample.ridden_boar)
        .and_then(|boar| boar.motion)
        .expect("motion");
    assert!(held.frame > 4.0);
    assert_eq!(held.index, 7);
}
