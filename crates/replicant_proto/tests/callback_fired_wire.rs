use replicant_proto::{
    CallbackFired, MAX_MESSAGE_BYTES, Message, PROTOCOL_VERSION, WireError, decode, encode,
};

fn fired() -> CallbackFired {
    CallbackFired {
        phase: 25,
        name: "Boss_Battle_Start".to_owned(),
    }
}

#[test]
fn a_callback_fired_round_trips_fits_a_message_and_is_relayed() {
    let message = Message::CallbackFired(fired());
    assert!(message.is_relayed());
    assert_eq!(message.validate(), Ok(()));
    let bytes = encode(&message).unwrap();
    assert!(bytes.len() <= MAX_MESSAGE_BYTES);
    assert_eq!(decode(&bytes), Ok(message));
    assert_eq!(PROTOCOL_VERSION, 67);
}

#[test]
fn a_callback_fired_with_a_bad_name_is_refused() {
    for name in [String::new(), "two words".to_owned(), "x".repeat(64)] {
        let bad = CallbackFired { name, ..fired() };
        assert_eq!(
            bad.validate(),
            Err(WireError::OutOfRange {
                field: "callback fired name"
            })
        );
    }
}

#[test]
fn a_watched_callback_fired_round_trips_is_relayed_and_validates_its_name() {
    let message = Message::WatchedCallbackFired(fired());
    assert!(message.is_relayed());
    assert_eq!(message.validate(), Ok(()));
    assert_eq!(decode(&encode(&message).unwrap()), Ok(message));
    let bad = Message::WatchedCallbackFired(CallbackFired {
        name: String::new(),
        ..fired()
    });
    assert!(bad.validate().is_err());
}

#[test]
fn a_stand_in_trap_fired_round_trips_is_relayed_and_validates_its_name() {
    let message = Message::StandInTrapFired(fired());
    assert!(message.is_relayed());
    assert_eq!(message.kind_name(), "StandInTrapFired");
    assert_eq!(message.validate(), Ok(()));
    assert_eq!(decode(&encode(&message).unwrap()), Ok(message));
    let bad = Message::StandInTrapFired(CallbackFired {
        name: String::new(),
        ..fired()
    });
    assert!(bad.validate().is_err());
}
