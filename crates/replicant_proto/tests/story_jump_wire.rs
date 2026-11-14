use replicant_proto::{Message, StoryJump, StoryJumpPoint, decode, encode};

#[test]
fn a_story_jump_round_trips_and_is_relayed() {
    let message = Message::StoryJump(StoryJump {
        point: StoryJumpPoint::AfterPrologue,
    });
    assert!(message.is_relayed());
    assert_eq!(message.validate(), Ok(()));
    assert_eq!(decode(&encode(&message).unwrap()), Ok(message));
}

#[test]
fn the_popola_talk_jump_round_trips_and_is_relayed() {
    let message = Message::StoryJump(StoryJump {
        point: StoryJumpPoint::AtPopolaTalk,
    });
    assert!(message.is_relayed());
    assert_eq!(message.validate(), Ok(()));
    assert_eq!(decode(&encode(&message).unwrap()), Ok(message));
}

#[test]
fn the_next_day_jump_round_trips_and_is_relayed() {
    let message = Message::StoryJump(StoryJump {
        point: StoryJumpPoint::BeforeNextDay,
    });
    assert!(message.is_relayed());
    assert_eq!(message.validate(), Ok(()));
    assert_eq!(decode(&encode(&message).unwrap()), Ok(message));
}

#[test]
fn a_teleport_here_round_trips_and_is_relayed() {
    let message = Message::TeleportHere(replicant_proto::TeleportHere {
        phase: 7,
        position: [1.5, 2.0, -3.25],
    });
    assert!(message.is_relayed());
    assert_eq!(message.validate(), Ok(()));
    assert_eq!(decode(&encode(&message).unwrap()), Ok(message));
}

#[test]
fn a_teleport_here_with_a_non_finite_position_is_refused() {
    let message = Message::TeleportHere(replicant_proto::TeleportHere {
        phase: 7,
        position: [f32::NAN, 0.0, 0.0],
    });
    assert!(message.validate().is_err());
}

#[test]
fn the_boss_fight_3_jump_round_trips_and_is_relayed() {
    let message = Message::StoryJump(StoryJump {
        point: StoryJumpPoint::BossFight3,
    });
    assert!(message.is_relayed());
    assert_eq!(message.validate(), Ok(()));
    assert_eq!(decode(&encode(&message).unwrap()), Ok(message));
}

#[test]
fn the_junk_heap_jump_round_trips_and_is_relayed() {
    let message = Message::StoryJump(StoryJump {
        point: StoryJumpPoint::JunkHeapFloorB2,
    });
    assert!(message.is_relayed());
    assert_eq!(message.validate(), Ok(()));
    assert_eq!(decode(&encode(&message).unwrap()), Ok(message));
}

#[test]
fn the_boss_fight_5_jump_round_trips_and_is_relayed() {
    let message = Message::StoryJump(StoryJump {
        point: StoryJumpPoint::JunkHeapBossFight5,
    });
    assert!(message.is_relayed());
    assert_eq!(message.validate(), Ok(()));
    assert_eq!(decode(&encode(&message).unwrap()), Ok(message));
}

#[test]
fn the_after_boss_fight_6_jump_round_trips_and_is_relayed() {
    let message = Message::StoryJump(StoryJump {
        point: StoryJumpPoint::AfterBossFight6,
    });
    assert!(message.is_relayed());
    assert_eq!(message.validate(), Ok(()));
    assert_eq!(decode(&encode(&message).unwrap()), Ok(message));
}

#[test]
fn the_older_jump_points_keep_their_encoded_index() {
    for (index, point) in [
        StoryJumpPoint::AfterPrologue,
        StoryJumpPoint::AtPopolaTalk,
        StoryJumpPoint::BeforeNextDay,
        StoryJumpPoint::AtShrineEntrance,
        StoryJumpPoint::AtShrineTop,
        StoryJumpPoint::AtShrineBoss,
        StoryJumpPoint::BossFight3,
        StoryJumpPoint::JunkHeapFloorB2,
        StoryJumpPoint::JunkHeapBossFight5,
        StoryJumpPoint::AfterBossFight6,
        StoryJumpPoint::BossFight6,
    ]
    .into_iter()
    .enumerate()
    {
        let bytes = postcard::to_allocvec(&point).unwrap();
        assert_eq!(bytes, vec![u8::try_from(index).unwrap()]);
    }
}

#[test]
fn a_gather_for_scene_round_trips_and_is_relayed() {
    let message = Message::GatherForScene(replicant_proto::GatherForScene {
        phase: 7,
        position: [1.5, 2.0, -3.25],
        in_side_view: true,
        side_view_door: 3,
    });
    assert!(message.is_relayed());
    assert_eq!(message.validate(), Ok(()));
    assert_eq!(decode(&encode(&message).unwrap()), Ok(message));
}

#[test]
fn a_gather_for_scene_with_a_non_finite_position_is_refused() {
    let message = Message::GatherForScene(replicant_proto::GatherForScene {
        phase: 7,
        position: [0.0, f32::INFINITY, 0.0],
        in_side_view: false,
        side_view_door: 0,
    });
    assert!(message.validate().is_err());
}

#[test]
fn every_gather_answer_round_trips_and_is_relayed() {
    for reply in [
        replicant_proto::GatherReply::Ready,
        replicant_proto::GatherReply::InAnotherArea,
        replicant_proto::GatherReply::InOwnTalk,
        replicant_proto::GatherReply::InOwnScene,
        replicant_proto::GatherReply::Downed,
        replicant_proto::GatherReply::SideViewNotGiven,
        replicant_proto::GatherReply::DidNotArrive,
    ] {
        let message = Message::GatherAnswer(replicant_proto::GatherAnswer { phase: 7, reply });
        assert!(message.is_relayed());
        assert_eq!(message.validate(), Ok(()));
        assert_eq!(decode(&encode(&message).unwrap()), Ok(message));
    }
}
