use replicant_proto::{BlockHeld, MAX_BLOCK_ID, Message, PROTOCOL_VERSION, decode, encode};

fn held(id: u16, held: bool) -> Message {
    Message::BlockHeld(BlockHeld { phase: 4, id, held })
}

#[test]
fn a_block_held_round_trips_and_is_relayed() {
    for flag in [true, false] {
        let message = held(2, flag);
        assert!(message.is_relayed());
        assert_eq!(message.validate(), Ok(()));
        assert_eq!(decode(&encode(&message).unwrap()), Ok(message));
    }
}

#[test]
fn a_block_held_with_an_id_past_the_limit_is_refused() {
    assert_eq!(held(MAX_BLOCK_ID, true).validate(), Ok(()));
    assert!(held(MAX_BLOCK_ID + 1, true).validate().is_err());
}

#[test]
fn the_block_held_message_round_trips_at_the_current_protocol() {
    assert_eq!(PROTOCOL_VERSION, 67);
}
