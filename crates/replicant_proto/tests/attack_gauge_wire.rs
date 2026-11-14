use replicant_proto::{
    AttackGaugeHit, AttackGaugeNode, AttackGaugeOpen, AttackGaugeState, EnemyId,
    MAX_ATTACK_GAUGE_NODES, MAX_MESSAGE_BYTES, Message, PROTOCOL_VERSION, decode, encode,
};

fn node() -> AttackGaugeNode {
    AttackGaugeNode {
        bone: 0x28,
        size: 0.6,
        offset: [0.25, -0.5, 1.0, 0.0],
        kind: 2,
        extra: 1,
        hp: -40_000,
        max_hp: 999_999,
    }
}

fn state(nodes: usize) -> AttackGaugeState {
    AttackGaugeState {
        phase: 23,
        serial: u32::MAX,
        open: Some(AttackGaugeOpen {
            boss: EnemyId {
                group: 1023,
                ordinal: 65_535,
            },
            partner: Some(EnemyId {
                group: 1022,
                ordinal: 65_534,
            }),
            total_seconds: 600.0,
            remaining_seconds: 599.5,
            map_points: [i32::MIN, i32::MAX, -1],
            options: [255, 255],
            nodes: vec![node(); nodes],
        }),
    }
}

#[test]
fn a_full_gauge_state_round_trips_fits_a_message_and_is_relayed() {
    let message = Message::AttackGauge(state(MAX_ATTACK_GAUGE_NODES));
    assert!(message.is_relayed());
    assert_eq!(message.validate(), Ok(()));
    let bytes = encode(&message).unwrap();
    assert!(bytes.len() <= MAX_MESSAGE_BYTES);
    assert_eq!(decode(&bytes), Ok(message));
    assert_eq!(PROTOCOL_VERSION, 67);
}

#[test]
fn a_closed_gauge_and_a_gauge_hit_round_trip_and_are_relayed() {
    let closed = Message::AttackGauge(AttackGaugeState {
        phase: 23,
        serial: 4,
        open: None,
    });
    let hit = Message::AttackGaugeHit(AttackGaugeHit {
        phase: 23,
        serial: 4,
        node: 7,
        damage: 6,
        seq: 9,
    });
    for message in [closed, hit] {
        assert!(message.is_relayed());
        assert_eq!(message.validate(), Ok(()));
        assert_eq!(decode(&encode(&message).unwrap()), Ok(message));
    }
}

#[test]
fn a_gauge_with_too_many_nodes_or_a_bad_clock_or_shape_is_refused() {
    assert!(
        Message::AttackGauge(state(MAX_ATTACK_GAUGE_NODES + 1))
            .validate()
            .is_err()
    );
    let mut bad_clock = state(1);
    if let Some(open) = &mut bad_clock.open {
        open.remaining_seconds = f32::NAN;
    }
    assert!(bad_clock.validate().is_err());
    let mut bad_shape = state(1);
    if let Some(open) = &mut bad_shape.open {
        open.nodes[0].offset[1] = f32::INFINITY;
    }
    assert!(bad_shape.validate().is_err());
}

#[test]
fn a_gauge_hit_needs_some_damage_on_a_node_the_wire_carries() {
    let hit = |node: u8, damage: i32| AttackGaugeHit {
        phase: 23,
        serial: 1,
        node,
        damage,
        seq: 1,
    };
    assert!(hit(0, 0).validate().is_err());
    assert!(hit(MAX_ATTACK_GAUGE_NODES as u8, 6).validate().is_err());
    assert_eq!(hit(0, 6).validate(), Ok(()));
}
