use replicant_proto::{
    ENEMY_GROUP_COUNT, EnemyBodyGone, EnemyDrop, EnemyHit, EnemyId, EnemyKilled, EnemyWordDrop,
    MAX_HIT_DAMAGE, MAX_HIT_PUSH_STRENGTH, MAX_ITEM_COUNT, MAX_MESSAGE_BYTES, MAX_WORD_ID, Message,
    PlayerHit, decode, encode,
};

fn id() -> EnemyId {
    EnemyId {
        group: 300,
        ordinal: 4,
    }
}

fn enemy_hit() -> EnemyHit {
    EnemyHit {
        phase: 12,
        id: id(),
        hit_id: u32::MAX,
        hp_damage: MAX_HIT_DAMAGE,
        second_gauge: MAX_HIT_DAMAGE,
        third_gauge: MAX_HIT_DAMAGE,
        flags: u32::MAX,
        attack_kind: u32::MAX,
    }
}

fn player_hit() -> PlayerHit {
    PlayerHit {
        phase: 12,
        attacker: id(),
        hp_damage: 40,
        flags: 0x2001000,
        attack_kind: 3,
        hit_id: 9,
        push: 0.0,
        direction: [0.0; 3],
    }
}

fn enemy_drop() -> EnemyDrop {
    EnemyDrop {
        phase: 12,
        id: id(),
        item_id: 0x150,
        count: 1,
        position: [12.5, -3.0, 400.0],
    }
}

#[test]
fn combat_messages_survive_a_round_trip_and_fit() {
    let messages = [
        Message::EnemyHit(enemy_hit()),
        Message::PlayerHit(player_hit()),
        Message::EnemyKilled(EnemyKilled {
            phase: 12,
            id: id(),
            by_guest: true,
        }),
        Message::EnemyDrop(enemy_drop()),
    ];
    for message in messages {
        let bytes = encode(&message).expect("valid message encodes");
        assert!(bytes.len() <= MAX_MESSAGE_BYTES);
        assert_eq!(decode(&bytes).expect("decodes"), message);
        assert!(message.is_relayed(), "{message:?}");
        assert!(!message.is_latest_wins(), "{message:?}");
    }
}

#[test]
fn hits_above_the_damage_cap_are_refused() {
    let too_hard = EnemyHit {
        hp_damage: MAX_HIT_DAMAGE + 1,
        ..enemy_hit()
    };
    assert!(encode(&Message::EnemyHit(too_hard)).is_err());
    let too_hard_on_player = PlayerHit {
        hp_damage: MAX_HIT_DAMAGE + 1,
        ..player_hit()
    };
    assert!(encode(&Message::PlayerHit(too_hard_on_player)).is_err());
}

#[test]
fn combat_messages_name_a_real_enemy_group() {
    let bad_id = EnemyId {
        group: u16::MAX,
        ordinal: 0,
    };
    assert!(
        encode(&Message::EnemyHit(EnemyHit {
            id: bad_id,
            ..enemy_hit()
        }))
        .is_err()
    );
    assert!(
        encode(&Message::EnemyKilled(EnemyKilled {
            phase: 1,
            id: bad_id,
            by_guest: false
        }))
        .is_err()
    );
}

#[test]
fn a_drop_needs_a_count_and_a_usable_position() {
    for bad in [
        EnemyDrop {
            count: 0,
            ..enemy_drop()
        },
        EnemyDrop {
            count: MAX_ITEM_COUNT + 1,
            ..enemy_drop()
        },
        EnemyDrop {
            position: [f32::NAN, 0.0, 0.0],
            ..enemy_drop()
        },
    ] {
        assert!(encode(&Message::EnemyDrop(bad)).is_err(), "{bad:?}");
    }
}

#[test]
fn a_word_drop_survives_a_round_trip_and_needs_a_word_id_in_range() {
    let drop = EnemyWordDrop {
        phase: 12,
        id: id(),
        word_id: 5,
    };
    let bytes = encode(&Message::EnemyWordDrop(drop)).expect("valid message encodes");
    assert!(bytes.len() <= MAX_MESSAGE_BYTES);
    assert_eq!(
        decode(&bytes).expect("decodes"),
        Message::EnemyWordDrop(drop)
    );
    let bad = EnemyWordDrop {
        word_id: MAX_WORD_ID + 1,
        ..drop
    };
    assert!(encode(&Message::EnemyWordDrop(bad)).is_err());
}

#[test]
fn a_body_gone_survives_a_round_trip_and_needs_a_group_in_range() {
    let gone = EnemyBodyGone {
        phase: 12,
        id: id(),
    };
    let bytes = encode(&Message::EnemyBodyGone(gone)).expect("valid message encodes");
    assert_eq!(
        decode(&bytes).expect("decodes"),
        Message::EnemyBodyGone(gone)
    );
    assert!(Message::EnemyBodyGone(gone).is_relayed());
    let bad = EnemyBodyGone {
        id: EnemyId {
            group: ENEMY_GROUP_COUNT,
            ordinal: 0,
        },
        ..gone
    };
    assert!(encode(&Message::EnemyBodyGone(bad)).is_err());
}

#[test]
fn a_hit_on_the_guest_carries_the_record_s_push_and_direction() {
    // The run's wide shot of 100:1: flags 0x800103, so the guest's game picks reaction kind 5.
    let thrown = PlayerHit {
        flags: 0x80_0103,
        push: 3.5,
        direction: [0.6, 0.0, -0.8],
        ..player_hit()
    };
    let bytes = encode(&Message::PlayerHit(thrown)).expect("valid message encodes");
    assert_eq!(decode(&bytes).expect("decodes"), Message::PlayerHit(thrown));
    for bad in [
        PlayerHit {
            push: -1.0,
            ..thrown
        },
        PlayerHit {
            push: MAX_HIT_PUSH_STRENGTH * 2.0,
            ..thrown
        },
        PlayerHit {
            direction: [f32::NAN, 0.0, 0.0],
            ..thrown
        },
    ] {
        assert!(encode(&Message::PlayerHit(bad)).is_err());
    }
}
