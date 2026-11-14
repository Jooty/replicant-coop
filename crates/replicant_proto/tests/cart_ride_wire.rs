use replicant_proto::{
    CartRide, MAX_CART_STATE, MAX_DOOR_LAYOUT_ID, MAX_MESSAGE_BYTES, Message, PROTOCOL_VERSION,
    PartnerNotice, WireError, decode, encode,
};

fn started_ride() -> CartRide {
    CartRide {
        phase: 13,
        layout_id: Some(41),
        cart_position: [10.2, 0.0, -105.6],
        from_state: 1,
        to_state: 5,
        boarded_at: [12.0, 0.0, -115.0],
    }
}

#[test]
fn the_protocol_number_went_up_with_the_boarding_spot() {
    assert_eq!(PROTOCOL_VERSION, 67);
}

#[test]
fn a_cart_ride_round_trips_and_is_relayed() {
    let message = Message::CartRide(started_ride());
    assert!(message.is_relayed());
    assert_eq!(message.kind_name(), "CartRide");
    let bytes = encode(&message).expect("encodes");
    assert!(bytes.len() < MAX_MESSAGE_BYTES / 8);
    assert_eq!(decode(&bytes), Ok(message));
}

#[test]
fn a_boarding_spot_far_from_the_cart_actor_is_carried() {
    let ride = CartRide {
        cart_position: [-37.0, -1.4, -8.4],
        boarded_at: [12.0, 0.0, -115.0],
        ..started_ride()
    };
    let bytes = encode(&Message::CartRide(ride)).expect("encodes");
    assert_eq!(decode(&bytes), Ok(Message::CartRide(ride)));
}

#[test]
fn a_cart_without_a_layout_id_still_round_trips() {
    let ride = CartRide {
        layout_id: None,
        ..started_ride()
    };
    let bytes = encode(&Message::CartRide(ride)).expect("encodes");
    assert_eq!(decode(&bytes), Ok(Message::CartRide(ride)));
}

#[test]
fn a_cart_ride_with_a_bad_state_place_spot_or_id_is_refused() {
    let no_state = CartRide {
        from_state: 0,
        ..started_ride()
    };
    let past_the_last_state = CartRide {
        to_state: MAX_CART_STATE + 1,
        ..started_ride()
    };
    let nowhere = CartRide {
        cart_position: [f32::NAN, 0.0, 0.0],
        ..started_ride()
    };
    let boarded_nowhere = CartRide {
        boarded_at: [0.0, f32::INFINITY, 0.0],
        ..started_ride()
    };
    let unknown_layout = CartRide {
        layout_id: Some(MAX_DOOR_LAYOUT_ID + 1),
        ..started_ride()
    };
    for (bad, field) in [
        (no_state, "cart_state"),
        (past_the_last_state, "cart_state"),
        (nowhere, "cart_position"),
        (boarded_nowhere, "boarded_at"),
        (unknown_layout, "layout_id"),
    ] {
        assert_eq!(
            encode(&Message::CartRide(bad)),
            Err(WireError::OutOfRange { field })
        );
    }
}

#[test]
fn a_guest_asking_to_ride_the_cart_is_a_relayed_notice() {
    let message = Message::Notice(PartnerNotice::WantsToRideTheCart);
    assert!(message.is_relayed());
    let bytes = encode(&message).expect("encodes");
    assert_eq!(decode(&bytes), Ok(message));
}
