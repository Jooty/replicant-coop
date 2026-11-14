use replicant_proto::{
    CartRideEnd, CartRun, CartRunProgress, MAX_CART_RUN_FRAME, MAX_CART_RUN_RATE,
    MAX_DOOR_LAYOUT_ID, MAX_MESSAGE_BYTES, Message, PROTOCOL_VERSION, WireError, decode, encode,
};

const CART_RUN_TAG: u8 = 113;

fn moving_run() -> CartRun {
    CartRun {
        phase: 13,
        layout_id: Some(41),
        cart_position: [10.2, 0.0, -105.6],
        run_state: 5,
        seq: 812,
        progress: CartRunProgress::Moving {
            frame: 1_234.5,
            rate: 1.7,
        },
    }
}

#[test]
fn the_protocol_number_went_up_with_the_cart_run() {
    assert_eq!(PROTOCOL_VERSION, 67);
}

#[test]
fn the_cart_run_comes_after_every_existing_variant() {
    let bytes = encode(&Message::CartRun(moving_run())).expect("encodes");
    assert_eq!(bytes[0], CART_RUN_TAG);
}

#[test]
fn a_cart_run_round_trips_small_and_relayed() {
    let message = Message::CartRun(moving_run());
    assert!(message.is_relayed());
    assert!(
        !message.is_latest_wins(),
        "ends must arrive: the run goes reliably and in order"
    );
    assert_eq!(message.kind_name(), "CartRun");
    let bytes = encode(&message).expect("encodes");
    assert!(bytes.len() < MAX_MESSAGE_BYTES / 16);
    assert_eq!(decode(&bytes), Ok(message));
}

#[test]
fn every_end_of_a_run_round_trips() {
    for end in [
        CartRideEnd::OnCartFlagCleared,
        CartRideEnd::CartParked,
        CartRideEnd::StateLeftRun,
        CartRideEnd::CartGone,
        CartRideEnd::PhaseChanged,
    ] {
        let message = Message::CartRun(CartRun {
            layout_id: None,
            progress: CartRunProgress::Ended(end),
            ..moving_run()
        });
        let bytes = encode(&message).expect("encodes");
        assert_eq!(decode(&bytes), Ok(message));
    }
}

#[test]
fn a_cart_run_with_a_bad_frame_rate_state_place_or_id_is_refused() {
    let moving = |frame: f32, rate: f32| CartRun {
        progress: CartRunProgress::Moving { frame, rate },
        ..moving_run()
    };
    for (bad, field) in [
        (moving(f32::NAN, 1.0), "frame"),
        (moving(-1.0, 1.0), "frame"),
        (moving(MAX_CART_RUN_FRAME + 1.0, 1.0), "frame"),
        (moving(10.0, f32::INFINITY), "rate"),
        (moving(10.0, -MAX_CART_RUN_RATE - 1.0), "rate"),
        (
            CartRun {
                run_state: 2,
                ..moving_run()
            },
            "run_state",
        ),
        (
            CartRun {
                run_state: 9,
                ..moving_run()
            },
            "run_state",
        ),
        (
            CartRun {
                cart_position: [0.0, f32::NAN, 0.0],
                ..moving_run()
            },
            "cart_position",
        ),
        (
            CartRun {
                layout_id: Some(MAX_DOOR_LAYOUT_ID + 1),
                ..moving_run()
            },
            "layout_id",
        ),
    ] {
        assert_eq!(
            encode(&Message::CartRun(bad)),
            Err(WireError::OutOfRange { field })
        );
    }
}
