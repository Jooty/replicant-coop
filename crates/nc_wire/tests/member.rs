use nc_wire::{MAX_SESSION_SLOTS, Member, STORY_MODE, SessionRules, Welcome};

#[test]
fn the_wire_ceiling_is_sixteen_slots_and_the_story_mode_is_zero() {
    assert_eq!(MAX_SESSION_SLOTS, 16);
    assert_eq!(STORY_MODE, 0);
}

#[test]
fn rules_are_the_mode_then_the_player_cap() {
    let rules = SessionRules {
        mode: 1,
        max_players: 8,
    };
    let bytes = postcard::to_stdvec(&rules).expect("serializes");
    assert_eq!(bytes, vec![1, 8]);
    assert_eq!(postcard::from_bytes::<SessionRules>(&bytes), Ok(rules));
}

#[test]
fn a_welcome_carries_the_rules() {
    let welcome = Welcome {
        version: 7,
        slot: 3,
        host_name: "Ho".to_owned(),
        rules: SessionRules {
            mode: STORY_MODE,
            max_players: 2,
        },
    };
    let bytes = postcard::to_stdvec(&welcome).expect("serializes");
    assert_eq!(bytes, vec![7, 3, 2, b'H', b'o', 0, 2]);
    assert_eq!(postcard::from_bytes::<Welcome>(&bytes), Ok(welcome));
}

#[test]
fn a_member_is_its_slot_then_its_name() {
    let member = Member {
        slot: 1,
        name: "Ka".to_owned(),
    };
    let bytes = postcard::to_stdvec(&member).expect("serializes");
    assert_eq!(bytes, vec![1, 2, b'K', b'a']);
    assert_eq!(postcard::from_bytes::<Member>(&bytes), Ok(member));
}
