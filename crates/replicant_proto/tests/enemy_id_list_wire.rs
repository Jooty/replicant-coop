use replicant_proto::{
    ENEMY_GROUP_COUNT, EnemyId, EnemyIdList, MAX_ENEMY_ID_LIST_PARTS, MAX_ENEMY_IDS_PER_LIST_PART,
    MAX_MESSAGE_BYTES, Message, WireError, decode, encode,
};

fn widest_id() -> EnemyId {
    EnemyId {
        group: ENEMY_GROUP_COUNT - 1,
        ordinal: u16::MAX,
    }
}

fn list(ids: Vec<EnemyId>) -> EnemyIdList {
    EnemyIdList {
        phase: 3,
        tick: 9,
        part: 0,
        parts: 1,
        ids,
    }
}

#[test]
fn an_id_list_survives_a_round_trip() {
    let message = Message::EnemyIdList(list(vec![
        EnemyId {
            group: 7,
            ordinal: 2,
        },
        EnemyId::companion(1),
    ]));
    let bytes = encode(&message).expect("valid message encodes");
    assert_eq!(decode(&bytes), Ok(message));
}

#[test]
fn a_full_id_list_part_of_the_widest_ids_fits_one_message() {
    let mut part = list(vec![widest_id(); MAX_ENEMY_IDS_PER_LIST_PART]);
    part.tick = u32::MAX;
    part.phase = u16::MAX;
    part.part = MAX_ENEMY_ID_LIST_PARTS - 1;
    part.parts = MAX_ENEMY_ID_LIST_PARTS;
    let bytes = encode(&Message::EnemyIdList(part)).expect("fits");
    assert!(bytes.len() <= MAX_MESSAGE_BYTES);
}

#[test]
fn an_id_list_with_bad_numbers_is_refused() {
    let too_many = list(vec![widest_id(); MAX_ENEMY_IDS_PER_LIST_PART + 1]);
    assert_eq!(
        encode(&Message::EnemyIdList(too_many)),
        Err(WireError::OutOfRange {
            field: "id_list_ids"
        })
    );
    for (part, parts) in [(0, 0), (1, 1), (0, MAX_ENEMY_ID_LIST_PARTS + 1)] {
        let bad = EnemyIdList {
            part,
            parts,
            ..list(Vec::new())
        };
        assert!(encode(&Message::EnemyIdList(bad)).is_err());
    }
    let unknown_group = list(vec![EnemyId {
        group: ENEMY_GROUP_COUNT,
        ordinal: 0,
    }]);
    assert!(encode(&Message::EnemyIdList(unknown_group)).is_err());
}

#[test]
fn an_id_list_is_a_relayed_latest_wins_message() {
    let message = Message::EnemyIdList(list(Vec::new()));
    assert!(message.is_relayed());
    assert!(message.is_latest_wins());
}
