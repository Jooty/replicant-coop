use nc_wire::{STORY_MODE, SessionRules};
use replicant_proto::{
    BuildGate, CombatEvent, Hello, MAX_DISPLAY_NAME_BYTES, MAX_MESSAGE_BYTES,
    MAX_RIDDEN_BOAR_OFFSET_ABS, MAX_WEAPON_ID, MAX_WEAPON_LEVEL, Message, MotionState,
    PROTOCOL_VERSION, PartnerActivity, Ping, PlayerState, Reject, RejectReason, RiddenBoar,
    WeaponState, Welcome, WireError, decode, encode,
};

const BUILD: BuildGate = BuildGate {
    time_date_stamp: 0x60B7_3C00,
    size_of_image: 0x0533_9000,
};

fn state() -> PlayerState {
    PlayerState {
        tick: 7,
        phase: 3,
        position: [1.5, -2.0, 300.25],
        yaw_radians: 1.25,
        motion: Some(MotionState {
            index: 12,
            frame: 0.5,
            speed: 1.0,
            looping: true,
            name_hash: 0,
        }),
        hp: 80,
        max_hp: 100,
        weapon: Some(weapon()),
        activity: PartnerActivity::InMenu,
        mp: 0,
        max_mp: 0,
        level: 0,
        ridden_boar: None,
    }
}

fn weapon() -> WeaponState {
    WeaponState {
        id: 4,
        level: 2,
        in_hand: true,
        visible: true,
    }
}

fn motion() -> MotionState {
    MotionState {
        index: 12,
        frame: 0.5,
        speed: 1.0,
        looping: true,
        name_hash: 0,
    }
}

fn hello() -> Hello {
    Hello {
        version: PROTOCOL_VERSION,
        build: BUILD,
        name: "Nier".to_owned(),
    }
}

fn round_trip(message: Message) {
    let bytes = encode(&message).expect("valid message encodes");
    assert!(bytes.len() <= MAX_MESSAGE_BYTES);
    assert_eq!(decode(&bytes), Ok(message));
}

#[test]
fn every_message_kind_survives_a_round_trip() {
    round_trip(Message::Hello(hello()));
    round_trip(Message::Welcome(Welcome {
        version: PROTOCOL_VERSION,
        slot: 1,
        host_name: "Host".to_owned(),
        rules: SessionRules {
            mode: STORY_MODE,
            max_players: 2,
        },
    }));
    round_trip(Message::Reject(Reject {
        reason: RejectReason::BuildMismatch,
    }));
    round_trip(Message::State(state()));
    round_trip(Message::Ping(Ping { nonce: 9 }));
    round_trip(Message::Bye);
    round_trip(Message::Combat(CombatEvent::Downed));
    round_trip(Message::Combat(CombatEvent::Revived));
    round_trip(Message::Combat(CombatEvent::GameOver));
    round_trip(Message::Combat(CombatEvent::Up));
    round_trip(Message::Combat(CombatEvent::ReviveProgress(0)));
    round_trip(Message::Combat(CombatEvent::ReviveProgress(100)));
    round_trip(Message::Combat(CombatEvent::HostCheckpoint));
    round_trip(Message::Combat(CombatEvent::HostGameOver));
    round_trip(Message::PathInfo(replicant_proto::PathInfo {
        rtt_micros: 1500,
        direct: true,
    }));
    round_trip(Message::Reject(Reject {
        reason: RejectReason::SessionFull,
    }));
    for readiness in [
        replicant_proto::GameReadiness::NotLinked,
        replicant_proto::GameReadiness::Loading,
        replicant_proto::GameReadiness::InPlay,
    ] {
        round_trip(Message::Readiness(readiness));
    }
    round_trip(Message::Notice(
        replicant_proto::PartnerNotice::WantsToChangeZone,
    ));
    round_trip(Message::Notice(
        replicant_proto::PartnerNotice::RejoiningYourArea,
    ));
}

#[test]
fn a_partner_notice_is_relayed_between_games() {
    assert!(Message::Notice(replicant_proto::PartnerNotice::WantsToChangeZone).is_relayed());
}

#[test]
fn a_readiness_message_is_relayed_between_games() {
    assert!(Message::Readiness(replicant_proto::GameReadiness::InPlay).is_relayed());
}

#[test]
fn a_state_message_fits_far_below_the_size_limit() {
    let bytes = encode(&Message::State(state())).expect("encodes");
    assert!(bytes.len() < MAX_MESSAGE_BYTES / 4);
}

#[test]
fn an_empty_message_is_refused() {
    assert_eq!(decode(&[]), Err(WireError::Empty));
}

#[test]
fn an_oversized_message_is_refused_before_parsing() {
    let bytes = vec![0u8; MAX_MESSAGE_BYTES + 1];
    assert!(matches!(decode(&bytes), Err(WireError::TooLarge { .. })));
}

#[test]
fn every_truncation_of_a_valid_message_is_refused() {
    let bytes = encode(&Message::State(state())).expect("encodes");
    for len in 1..bytes.len() {
        assert!(
            decode(&bytes[..len]).is_err(),
            "prefix of {len} bytes decoded"
        );
    }
}

#[test]
fn bytes_after_the_message_are_refused() {
    let mut bytes = encode(&Message::Bye).expect("encodes");
    bytes.push(0);
    assert_eq!(decode(&bytes), Err(WireError::TrailingBytes { extra: 1 }));
}

#[test]
fn an_unknown_message_tag_is_refused() {
    assert_eq!(decode(&[0xFF, 0x01]), Err(WireError::Malformed));
}

#[test]
fn garbage_never_panics_and_never_decodes_to_an_invalid_message() {
    let mut seed: u32 = 0x1234_5678;
    for _ in 0..2000 {
        let len = {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 24) as usize % 64 + 1
        };
        let bytes: Vec<u8> = (0..len)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 24) as u8
            })
            .collect();
        if let Ok(message) = decode(&bytes) {
            assert_eq!(message.validate(), Ok(()));
        }
    }
}

#[test]
fn out_of_range_player_state_is_refused_on_encode_and_decode() {
    let bad_states = [
        PlayerState { hp: 101, ..state() },
        PlayerState {
            mp: 5,
            max_mp: 4,
            ..state()
        },
        PlayerState {
            position: [f32::NAN, 0.0, 0.0],
            ..state()
        },
        PlayerState {
            position: [0.0, f32::INFINITY, 0.0],
            ..state()
        },
        PlayerState {
            position: [2_000_000.0, 0.0, 0.0],
            ..state()
        },
        PlayerState {
            yaw_radians: 100.0,
            ..state()
        },
        PlayerState {
            motion: Some(MotionState {
                frame: -1.0,
                ..motion()
            }),
            ..state()
        },
        PlayerState {
            motion: Some(MotionState {
                speed: f32::NAN,
                ..motion()
            }),
            ..state()
        },
        PlayerState {
            motion: Some(MotionState {
                speed: 1000.0,
                ..motion()
            }),
            ..state()
        },
        PlayerState {
            max_hp: u32::MAX,
            hp: 0,
            ..state()
        },
        PlayerState {
            weapon: Some(WeaponState {
                id: MAX_WEAPON_ID + 1,
                ..weapon()
            }),
            ..state()
        },
        PlayerState {
            weapon: Some(WeaponState {
                level: MAX_WEAPON_LEVEL + 1,
                ..weapon()
            }),
            ..state()
        },
    ];
    for bad in bad_states {
        assert!(matches!(
            encode(&Message::State(bad)),
            Err(WireError::OutOfRange { .. })
        ));
        let raw = postcard::to_stdvec(&Message::State(bad)).expect("serialises");
        assert!(matches!(decode(&raw), Err(WireError::OutOfRange { .. })));
    }
}

#[test]
fn display_names_are_bounded_and_printable() {
    let too_long = "x".repeat(MAX_DISPLAY_NAME_BYTES + 1);
    for name in [too_long.as_str(), "", "tab\there"] {
        let message = Message::Hello(Hello {
            name: name.to_owned(),
            ..hello()
        });
        assert!(matches!(
            encode(&message),
            Err(WireError::OutOfRange { field: "name" })
        ));
    }
    let longest = "x".repeat(MAX_DISPLAY_NAME_BYTES);
    round_trip(Message::Hello(Hello {
        name: longest,
        ..hello()
    }));
}

#[test]
fn a_hello_from_another_protocol_version_still_decodes_so_the_session_can_reject_it() {
    let message = Message::Hello(Hello {
        version: PROTOCOL_VERSION + 1,
        ..hello()
    });
    round_trip(message);
}

#[test]
fn a_welcome_for_a_slot_beyond_the_player_limit_is_refused() {
    let message = Message::Welcome(Welcome {
        version: PROTOCOL_VERSION,
        slot: 2,
        host_name: "Host".to_owned(),
        rules: SessionRules {
            mode: STORY_MODE,
            max_players: 2,
        },
    });
    assert!(matches!(
        encode(&message),
        Err(WireError::OutOfRange { field: "slot" })
    ));
}

#[test]
fn the_largest_state_stays_under_the_message_cap() {
    let largest = PlayerState {
        tick: u32::MAX,
        phase: u16::MAX,
        position: [-999_999.9; 3],
        yaw_radians: -15.9,
        motion: Some(MotionState {
            index: u16::MAX,
            frame: 99_999.9,
            speed: -31.9,
            looping: true,
            name_hash: 0,
        }),
        hp: 999_999,
        max_hp: 999_999,
        weapon: Some(WeaponState {
            id: MAX_WEAPON_ID,
            level: MAX_WEAPON_LEVEL,
            in_hand: true,
            visible: true,
        }),
        activity: PartnerActivity::InCutscene,
        mp: 0,
        max_mp: 0,
        level: u8::MAX,
        ridden_boar: Some(RiddenBoar {
            offset_from_rider: [-MAX_RIDDEN_BOAR_OFFSET_ABS; 3],
            yaw_from_rider_radians: -15.9,
            motion: Some(MotionState {
                index: u16::MAX,
                frame: 99_999.9,
                speed: -31.9,
                looping: true,
                name_hash: u32::MAX,
            }),
        }),
    };
    let bytes = encode(&Message::State(largest)).expect("encodes");
    assert!(bytes.len() < MAX_MESSAGE_BYTES / 2);
}

#[test]
fn an_item_pickup_round_trips_and_is_relayed() {
    let pickup = Message::Pickup(replicant_proto::ItemPickup {
        item_id: 0x100,
        count: 1,
        source: replicant_proto::PickupSource::World,
    });
    round_trip(pickup.clone());
    assert!(pickup.is_relayed());
}

#[test]
fn an_item_pickup_outside_the_item_table_or_empty_is_refused() {
    let bad_id = replicant_proto::ItemPickup {
        item_id: 0x300,
        count: 1,
        source: replicant_proto::PickupSource::Shop,
    };
    let empty = replicant_proto::ItemPickup {
        item_id: 1,
        count: 0,
        source: replicant_proto::PickupSource::Other,
    };
    assert!(Message::Pickup(bad_id).validate().is_err());
    assert!(Message::Pickup(empty).validate().is_err());
}
