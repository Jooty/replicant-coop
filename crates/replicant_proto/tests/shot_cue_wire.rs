use replicant_proto::{
    MAX_BULLET_INSTANCE, MAX_MESSAGE_BYTES, MAX_SHOT_CUE_ID, Message, ShotCue, ShotCueKind,
    WireError, decode, encode,
};

fn impact() -> ShotCue {
    let mut transform = [0.0_f32; 12];
    transform[0] = 1.0;
    transform[5] = 1.0;
    transform[10] = 1.0;
    transform[3] = -18.3;
    transform[7] = 69.7;
    transform[11] = 0.6;
    ShotCue {
        instance: Some(300),
        kind: ShotCueKind::OneShotEffect,
        id: 8033,
        transform,
        offset: [0.0, -0.5, 0.0],
        style: [1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0],
        sound_levels: [0.0; 2],
    }
}

#[test]
fn a_shot_cue_round_trips_fits_a_message_and_is_relayed() {
    let message = Message::ShotCue(impact());
    assert!(message.is_relayed());
    assert_eq!(message.kind_name(), "ShotCue");
    assert_eq!(message.validate(), Ok(()));
    let bytes = encode(&message).unwrap();
    assert!(bytes.len() <= MAX_MESSAGE_BYTES);
    assert_eq!(decode(&bytes), Ok(message));
}

#[test]
fn a_shot_cue_with_no_copy_to_name_is_still_valid() {
    let cue = ShotCue {
        instance: None,
        ..impact()
    };
    assert_eq!(cue.validate(), Ok(()));
}

#[test]
fn a_shot_cue_out_of_range_is_refused() {
    let bad_instance = ShotCue {
        instance: Some(MAX_BULLET_INSTANCE + 1),
        ..impact()
    };
    let bad_id = ShotCue {
        id: MAX_SHOT_CUE_ID + 1,
        ..impact()
    };
    let mut bad_place = impact();
    bad_place.transform[7] = f32::NAN;
    let mut bad_style = impact();
    bad_style.sound_levels[1] = f32::INFINITY;
    assert_eq!(
        bad_instance.validate(),
        Err(WireError::OutOfRange {
            field: "shot cue instance"
        })
    );
    assert_eq!(
        bad_id.validate(),
        Err(WireError::OutOfRange {
            field: "shot cue id"
        })
    );
    assert_eq!(
        bad_place.validate(),
        Err(WireError::OutOfRange {
            field: "shot cue place"
        })
    );
    assert_eq!(
        bad_style.validate(),
        Err(WireError::OutOfRange {
            field: "shot cue style"
        })
    );
}

#[test]
fn only_effects_and_sounds_are_made_the_end_kinds_are_neither() {
    assert!(ShotCueKind::FollowingEffect.is_effect());
    assert!(ShotCueKind::OneShotEffect.is_effect());
    assert!(ShotCueKind::OneShotSound.is_sound());
    assert!(ShotCueKind::LastingSound.is_sound());
    for end in [
        ShotCueKind::EndLastingSound,
        ShotCueKind::EndFollowingEffects,
    ] {
        assert!(!end.is_effect() && !end.is_sound());
    }
}
