use replicant_proto::{
    COMPANION_ENEMY_GROUP, COMPANION_SLOT_COUNT, ENEMY_GROUP_COUNT, EnemyGone, EnemyGoneReason,
    EnemyHit, EnemyId, EnemySnapshot, EnemySpawned, EnemyState, MAX_ENEMIES_PER_SNAPSHOT,
    MAX_ENEMY_SNAPSHOT_PARTS, MAX_HP, MAX_MESSAGE_BYTES, MAX_MOTION_FRAME, MAX_MOTION_SPEED_ABS,
    MAX_POSITION_ABS, MAX_YAW_ABS_RADIANS, Message, MotionState, PlayerState, WireError, decode,
    encode,
};

fn enemy(group: u16, ordinal: u16) -> EnemyState {
    EnemyState {
        id: EnemyId { group, ordinal },
        position: [10.0, -2.5, 300.0],
        yaw_radians: 0.75,
        motion: Some(MotionState {
            index: 4,
            frame: 12.0,
            speed: 1.0,
            looping: true,
            name_hash: 0,
        }),
        hp: 40,
        max_hp: 120,
    }
}

fn widest_enemy() -> EnemyState {
    EnemyState {
        id: EnemyId {
            group: ENEMY_GROUP_COUNT - 1,
            ordinal: u16::MAX,
        },
        position: [-MAX_POSITION_ABS; 3],
        yaw_radians: -MAX_YAW_ABS_RADIANS,
        motion: Some(MotionState {
            index: u16::MAX,
            frame: MAX_MOTION_FRAME,
            speed: -MAX_MOTION_SPEED_ABS,
            looping: true,
            name_hash: 0,
        }),
        hp: MAX_HP,
        max_hp: MAX_HP,
    }
}

fn snapshot(enemies: Vec<EnemyState>) -> EnemySnapshot {
    EnemySnapshot {
        tick: 9,
        phase: 3,
        part: 0,
        parts: 1,
        total: u16::try_from(enemies.len()).expect("small"),
        round: u16::try_from(enemies.len()).expect("small"),
        enemies,
    }
}

#[test]
fn enemy_messages_survive_a_round_trip() {
    for message in [
        Message::EnemySnapshot(snapshot(vec![enemy(5, 0), enemy(5, 1)])),
        Message::EnemySpawned(EnemySpawned {
            phase: 3,
            enemy: enemy(7, 2),
        }),
        Message::EnemyGone(EnemyGone {
            phase: 3,
            id: EnemyId {
                group: 7,
                ordinal: 2,
            },
            reason: EnemyGoneReason::Killed,
        }),
    ] {
        let bytes = encode(&message).expect("valid message encodes");
        assert_eq!(decode(&bytes), Ok(message));
    }
}

#[test]
fn a_full_snapshot_part_of_the_widest_enemies_fits_one_message() {
    let mut part = snapshot(vec![widest_enemy(); MAX_ENEMIES_PER_SNAPSHOT]);
    part.tick = u32::MAX;
    part.phase = u16::MAX;
    part.part = MAX_ENEMY_SNAPSHOT_PARTS - 1;
    part.parts = MAX_ENEMY_SNAPSHOT_PARTS;
    part.total = u16::MAX;
    let bytes = encode(&Message::EnemySnapshot(part)).expect("fits");
    assert!(bytes.len() <= MAX_MESSAGE_BYTES);
}

#[test]
fn a_snapshot_part_with_too_many_enemies_is_refused() {
    let part = snapshot(vec![enemy(1, 0); MAX_ENEMIES_PER_SNAPSHOT + 1]);
    assert_eq!(
        encode(&Message::EnemySnapshot(part)),
        Err(WireError::OutOfRange {
            field: "snapshot_enemies"
        })
    );
}

#[test]
fn a_part_number_outside_the_part_count_is_refused() {
    for (part, parts) in [(0, 0), (1, 1), (0, MAX_ENEMY_SNAPSHOT_PARTS + 1)] {
        let bad = EnemySnapshot {
            part,
            parts,
            ..snapshot(Vec::new())
        };
        assert!(encode(&Message::EnemySnapshot(bad)).is_err());
    }
}

#[test]
fn an_enemy_with_bad_numbers_is_refused() {
    let bad = [
        EnemyState {
            id: EnemyId {
                group: ENEMY_GROUP_COUNT,
                ordinal: 0,
            },
            ..enemy(1, 0)
        },
        EnemyState {
            position: [f32::NAN, 0.0, 0.0],
            ..enemy(1, 0)
        },
        EnemyState {
            yaw_radians: f32::INFINITY,
            ..enemy(1, 0)
        },
        EnemyState {
            hp: 121,
            ..enemy(1, 0)
        },
    ];
    for state in bad {
        let spawned = Message::EnemySpawned(EnemySpawned {
            phase: 1,
            enemy: state,
        });
        assert!(encode(&spawned).is_err(), "{state:?}");
    }
}

#[test]
fn snapshots_go_unreliable_and_spawn_and_death_go_reliable() {
    let snapshot = Message::EnemySnapshot(snapshot(Vec::new()));
    let spawned = Message::EnemySpawned(EnemySpawned {
        phase: 1,
        enemy: enemy(1, 0),
    });
    assert!(snapshot.is_latest_wins());
    assert!(!spawned.is_latest_wins());
    assert!(snapshot.is_relayed() && spawned.is_relayed());
}

#[test]
fn player_state_is_not_relayed_as_a_game_message() {
    let state = Message::State(PlayerState {
        tick: 1,
        phase: 1,
        position: [0.0; 3],
        yaw_radians: 0.0,
        motion: None,
        hp: 1,
        max_hp: 1,
        weapon: None,
        activity: Default::default(),
        mp: 0,
        max_mp: 0,
        level: 0,
        ridden_boar: None,
    });
    assert!(!state.is_relayed());
    assert!(!Message::Bye.is_relayed());
}

#[test]
fn a_companion_has_a_fixed_id_outside_the_script_groups() {
    let id = EnemyId::companion(0);
    assert!(id.is_companion());
    assert_eq!(id.group, COMPANION_ENEMY_GROUP);
    const { assert!(COMPANION_ENEMY_GROUP >= ENEMY_GROUP_COUNT) };
    assert!(!enemy(ENEMY_GROUP_COUNT - 1, 0).id.is_companion());
}

#[test]
fn a_companion_rides_the_enemy_snapshot_and_takes_hits_by_its_id() {
    assert_eq!(replicant_proto::PROTOCOL_VERSION, 67);
    for slot in 0..COMPANION_SLOT_COUNT {
        let mut state = widest_enemy();
        state.id = EnemyId::companion(slot);
        let mut enemies = vec![widest_enemy(); MAX_ENEMIES_PER_SNAPSHOT - 1];
        enemies.push(state);
        let part = snapshot(enemies);
        let bytes = encode(&Message::EnemySnapshot(part.clone())).expect("fits");
        assert!(bytes.len() <= MAX_MESSAGE_BYTES);
        assert_eq!(
            decode(&bytes).expect("decodes"),
            Message::EnemySnapshot(part)
        );
        let hit = EnemyHit {
            phase: 3,
            id: EnemyId::companion(slot),
            hit_id: 9,
            hp_damage: 12,
            second_gauge: 0,
            third_gauge: 0,
            flags: 0,
            attack_kind: 1,
        };
        let bytes = encode(&Message::EnemyHit(hit)).expect("valid message encodes");
        assert_eq!(decode(&bytes).expect("decodes"), Message::EnemyHit(hit));
    }
}

#[test]
fn a_companion_slot_past_the_last_is_refused() {
    let mut state = enemy(1, 0);
    state.id = EnemyId::companion(COMPANION_SLOT_COUNT);
    assert!(encode(&Message::EnemySnapshot(snapshot(vec![state]))).is_err());
}

#[test]
fn companion_ids_sort_after_every_script_group_id() {
    let last_enemy = EnemyId {
        group: ENEMY_GROUP_COUNT - 1,
        ordinal: u16::MAX,
    };
    assert!(EnemyId::companion(0) > last_enemy);
    assert!(EnemyId::companion(COMPANION_SLOT_COUNT - 1) > EnemyId::companion(0));
}

#[test]
fn a_snapshot_whose_round_is_larger_than_the_host_total_is_refused() {
    let mut part = snapshot(Vec::new());
    part.total = 3;
    part.round = 4;
    assert_eq!(
        encode(&Message::EnemySnapshot(part)),
        Err(WireError::OutOfRange {
            field: "snapshot_round"
        })
    );
}

#[test]
fn a_snapshot_round_carries_both_numbers_through_a_round_trip() {
    let mut part = snapshot(vec![enemy(1, 0)]);
    part.total = 90;
    part.round = 80;
    let message = Message::EnemySnapshot(part);
    let bytes = encode(&message).expect("valid message encodes");
    assert_eq!(decode(&bytes), Ok(message));
}
