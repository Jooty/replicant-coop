use replicant_proto::{
    LiftCall, LiftCallAnswer, LiftCallOutcome, MAX_WARP_POINT_ID, Message, PROTOCOL_VERSION,
    WireError, decode, encode,
};

fn a_call() -> LiftCall {
    LiftCall {
        phase: 12,
        point: 401,
        origin: 400,
        position: [19.5, 0.0, -2.5],
    }
}

#[test]
fn the_protocol_number_went_up_with_the_lift_call() {
    assert_eq!(PROTOCOL_VERSION, 67);
}

#[test]
fn a_lift_call_and_its_answer_round_trip_and_are_relayed() {
    let call = Message::LiftCall(a_call());
    let answer = Message::LiftCallAnswer(LiftCallAnswer {
        phase: 12,
        point: 401,
        outcome: LiftCallOutcome::HostBusy,
    });
    for message in [call, answer] {
        assert!(message.is_relayed());
        let bytes = encode(&message).expect("encodes");
        assert_eq!(decode(&bytes), Ok(message));
    }
    assert_eq!(Message::LiftCall(a_call()).kind_name(), "LiftCall");
}

#[test]
fn a_lift_call_past_the_last_warp_point_or_nowhere_is_refused() {
    let past = LiftCall {
        origin: MAX_WARP_POINT_ID + 1,
        ..a_call()
    };
    let nowhere = LiftCall {
        position: [0.0, f32::INFINITY, 0.0],
        ..a_call()
    };
    assert_eq!(
        Message::LiftCall(past).validate(),
        Err(WireError::OutOfRange {
            field: "warp_point"
        })
    );
    assert_eq!(
        Message::LiftCall(nowhere).validate(),
        Err(WireError::OutOfRange { field: "position" })
    );
    assert_eq!(Message::LiftCall(a_call()).validate(), Ok(()));
}
