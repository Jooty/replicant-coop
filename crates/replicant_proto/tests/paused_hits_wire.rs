use replicant_proto::{Message, PROTOCOL_VERSION, PausedHits, decode, encode};

#[test]
fn a_paused_hits_choice_round_trips_and_is_relayed() {
    for paused_players_can_be_hit in [false, true] {
        let message = Message::PausedHits(PausedHits {
            paused_players_can_be_hit,
        });
        assert!(message.is_relayed());
        assert_eq!(message.validate(), Ok(()));
        assert_eq!(decode(&encode(&message).unwrap()), Ok(message));
    }
    assert_eq!(PROTOCOL_VERSION, 67);
}

#[test]
fn paused_players_are_safe_by_default() {
    assert!(!PausedHits::default().paused_players_can_be_hit);
}
