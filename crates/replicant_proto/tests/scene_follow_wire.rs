use replicant_proto::{
    MAX_MESSAGE_BYTES, MAX_SCENE_FOLLOW_CALLBACKS, Message, PROTOCOL_VERSION, SceneFollow,
    WireError, decode, encode,
};

fn follow() -> SceneFollow {
    SceneFollow {
        phase: 25,
        callbacks: vec!["EID_0050_a0030g_End".to_owned()],
    }
}

#[test]
fn a_scene_follow_round_trips_fits_a_message_and_is_relayed() {
    let message = Message::SceneFollow(follow());
    assert!(message.is_relayed());
    assert_eq!(message.validate(), Ok(()));
    let bytes = encode(&message).unwrap();
    assert!(bytes.len() <= MAX_MESSAGE_BYTES);
    assert_eq!(decode(&bytes), Ok(message));
    assert_eq!(PROTOCOL_VERSION, 67);
}

#[test]
fn a_bad_scene_follow_is_refused() {
    let mut many = follow();
    many.callbacks = vec!["A".to_owned(); MAX_SCENE_FOLLOW_CALLBACKS + 1];
    let mut empty_name = follow();
    empty_name.callbacks = vec![String::new()];
    let mut spaced = follow();
    spaced.callbacks = vec!["two words".to_owned()];
    let mut long = follow();
    long.callbacks = vec!["x".repeat(64)];
    assert_eq!(
        many.validate(),
        Err(WireError::OutOfRange {
            field: "scene follow callbacks"
        })
    );
    for bad in [empty_name, spaced, long] {
        assert_eq!(
            bad.validate(),
            Err(WireError::OutOfRange {
                field: "scene follow callback name"
            })
        );
    }
}
