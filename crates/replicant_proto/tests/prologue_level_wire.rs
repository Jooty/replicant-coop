use replicant_proto::{
    MAX_PROLOGUE_LEVEL, MAX_REPORTED_WEAPON_ID, MAX_REPORTED_WEAPON_LEVEL, Message,
    NO_WEAPON_REPORTED, PrologueLevel, WireError, decode, encode,
};

#[test]
fn a_prologue_level_round_trips_and_is_relayed() {
    let message = Message::PrologueLevel(PrologueLevel {
        level: 7,
        experience: 1234,
        weapon: 13,
        weapon_level: 2,
    });
    assert!(message.is_relayed());
    assert_eq!(message.validate(), Ok(()));
    assert_eq!(decode(&encode(&message).unwrap()), Ok(message));
}

#[test]
fn a_level_outside_the_range_is_refused() {
    for level in [-1, MAX_PROLOGUE_LEVEL + 1] {
        assert_eq!(
            PrologueLevel {
                level,
                experience: 0,
                weapon: NO_WEAPON_REPORTED,
                weapon_level: 0,
            }
            .validate(),
            Err(WireError::OutOfRange { field: "level" })
        );
    }
}

#[test]
fn negative_experience_is_refused() {
    assert_eq!(
        PrologueLevel {
            level: 1,
            experience: -1,
            weapon: NO_WEAPON_REPORTED,
            weapon_level: 0,
        }
        .validate(),
        Err(WireError::OutOfRange {
            field: "experience"
        })
    );
}

#[test]
fn a_weapon_outside_the_table_or_its_levels_is_refused() {
    let with = |weapon, weapon_level| PrologueLevel {
        level: 1,
        experience: 0,
        weapon,
        weapon_level,
    };
    assert_eq!(with(NO_WEAPON_REPORTED, 0).validate(), Ok(()));
    assert_eq!(
        with(MAX_REPORTED_WEAPON_ID, MAX_REPORTED_WEAPON_LEVEL).validate(),
        Ok(())
    );
    assert_eq!(
        with(MAX_REPORTED_WEAPON_ID + 1, 0).validate(),
        Err(WireError::OutOfRange { field: "weapon" })
    );
    assert_eq!(
        with(NO_WEAPON_REPORTED - 1, 0).validate(),
        Err(WireError::OutOfRange { field: "weapon" })
    );
    assert_eq!(
        with(13, MAX_REPORTED_WEAPON_LEVEL + 1).validate(),
        Err(WireError::OutOfRange {
            field: "weapon_level"
        })
    );
}
