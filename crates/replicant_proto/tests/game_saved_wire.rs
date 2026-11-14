use replicant_proto::{Message, PROTOCOL_VERSION, decode, encode};

const GAME_SAVED_TAG: u8 = 112;

#[test]
fn the_saved_message_is_part_of_the_current_protocol() {
    assert_eq!(PROTOCOL_VERSION, 67);
}

#[test]
fn the_saved_message_comes_after_every_existing_variant() {
    let bytes = encode(&Message::GameSaved { succeeded: true }).expect("encodes");
    assert_eq!(bytes[0], GAME_SAVED_TAG);
}

#[test]
fn the_saved_message_round_trips_either_result() {
    for succeeded in [true, false] {
        let message = Message::GameSaved { succeeded };
        let bytes = encode(&message).expect("encodes");
        assert_eq!(decode(&bytes), Ok(message));
    }
}

#[test]
fn the_saved_message_stays_on_the_local_link() {
    let message = Message::GameSaved { succeeded: true };
    assert!(!message.is_relayed());
    assert_eq!(message.validate(), Ok(()));
}
