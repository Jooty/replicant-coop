use replicant_game::kaine::{
    ApproachFlags, COMPANION_OFF_FIGHTS, COMPANION_OFF_FIGHTS_FROM_SCRIPTS, CompanionOff,
    CompanionOffApproach, CompanionOffEvent, CompanionOffFight, GuestSceneStandIn, RowOrigin,
    SCENE_STAND_INS, SCENE_STAND_INS_FROM_SCRIPTS, SceneStandIn, SceneStandInEvent,
    SceneStandInFacts, StandInSide, companion_off_fight_opened_by, companion_off_fights_approached,
    companion_off_fights_from_scripts_in_use, fight_from_scripts_opened_by,
    scene_stand_in_opened_by, scene_stand_ins_from_scripts_in_use,
};

const COPY_PACK: &str = "A_TRIAL_01";
const TEST_PACK: &str = "TEST_PACK_01";
const TEST_ON_FLAG: u16 = 10;
const TEST_START: &str = "EID_9999_a0010_Start";
static TEST_FIGHTS: [CompanionOffFight; 1] = [CompanionOffFight {
    name: "TEST_PACK_01 EID_9999 boss 1 (from the scripts)",
    origin: RowOrigin::FromScripts,
    start_functions: &[TEST_START],
    end_functions: &["EID_9999_a0020_Start"],
    approach: CompanionOffApproach {
        pack: TEST_PACK,
        flags: Some(ApproachFlags {
            on_game_flag: TEST_ON_FLAG,
            off_game_flag: 11,
        }),
    },
    functions_that_address_her: &[TEST_START],
    boss_group: Some(1),
}];

fn sorted<'a>(names: &[&'a str]) -> Vec<&'a str> {
    let mut names = names.to_vec();
    names.sort_unstable();
    names
}

fn generated_twin_of(hand: &CompanionOffFight) -> &'static CompanionOffFight {
    COMPANION_OFF_FIGHTS_FROM_SCRIPTS
        .iter()
        .find(|row| {
            row.approach.pack == hand.approach.pack
                && row
                    .start_functions
                    .iter()
                    .any(|name| hand.start_functions.contains(name))
        })
        .unwrap_or_else(|| panic!("the scripts give no row for {}", hand.name))
}

fn scene_row_in_use() -> &'static SceneStandIn {
    scene_stand_ins_from_scripts_in_use()
        .next()
        .expect("the scripts give at least one scene row no hand-made row lists")
}

fn guest_in(pack: &'static str, now_ms: u64) -> SceneStandInFacts<'static> {
    SceneStandInFacts {
        side: StandInSide::Guest,
        in_session: true,
        pack: Some(pack),
        closing_flag_on: false,
        free_to_move: true,
        now_ms,
    }
}

fn release_reason(events: &[SceneStandInEvent]) -> Option<&'static str> {
    events.iter().find_map(|event| match event {
        SceneStandInEvent::Released { why, .. } => Some(*why),
        _ => None,
    })
}

fn places(events: &[SceneStandInEvent]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, SceneStandInEvent::Place { .. }))
        .count()
}

fn scene_rule_from_scripts_in(pack: &'static str) -> GuestSceneStandIn {
    let mut rule = GuestSceneStandIn::new();
    rule.use_rows_from_scripts(true);
    assert!(rule.step(&guest_in(pack, 0)).is_empty());
    rule
}

#[test]
fn the_rows_from_the_scripts_reproduce_both_hand_made_fight_rows() {
    for hand in &COMPANION_OFF_FIGHTS {
        let row = generated_twin_of(hand);
        assert_eq!(
            sorted(row.start_functions),
            sorted(hand.start_functions),
            "{}",
            hand.name
        );
        assert_eq!(
            sorted(row.end_functions),
            sorted(hand.end_functions),
            "{}",
            hand.name
        );
        assert_eq!(row.approach, hand.approach, "{}", hand.name);
        for name in hand.functions_that_address_her {
            assert!(
                row.functions_that_address_her.contains(name),
                "{}: {name}",
                hand.name
            );
        }
    }
}

#[test]
fn the_rows_from_the_scripts_reproduce_the_hand_made_scene_row() {
    for hand in &SCENE_STAND_INS {
        let row = SCENE_STAND_INS_FROM_SCRIPTS
            .iter()
            .find(|row| row.talk_id == hand.talk_id && row.pack == hand.pack)
            .unwrap_or_else(|| panic!("the scripts give no row for {}", hand.scene));
        assert_eq!(row.ended_by, hand.ended_by);
        assert_eq!(row.closed_by_game_flag, hand.closed_by_game_flag);
        assert_eq!(row.script_spot, hand.script_spot);
        assert_eq!(row.script_heading_degrees, hand.script_heading_degrees);
        assert_eq!(row.companion_kept_for, hand.companion_kept_for);
    }
}

#[test]
fn every_row_says_where_it_comes_from_and_a_generated_name_is_its_ids() {
    for row in &COMPANION_OFF_FIGHTS {
        assert_eq!(row.origin, RowOrigin::HandMade, "{}", row.name);
    }
    for row in &COMPANION_OFF_FIGHTS_FROM_SCRIPTS {
        assert_eq!(row.origin, RowOrigin::FromScripts, "{}", row.name);
        assert!(row.name.starts_with(row.approach.pack), "{}", row.name);
        assert!(row.name.ends_with("(from the scripts)"), "{}", row.name);
        assert!(!row.start_functions.is_empty(), "{}", row.name);
        if let Some(flags) = row.approach.flags {
            assert_ne!(flags.on_game_flag, flags.off_game_flag, "{}", row.name);
        }
    }
    for row in &SCENE_STAND_INS {
        assert_eq!(row.origin, RowOrigin::HandMade, "{}", row.scene);
    }
    for row in &SCENE_STAND_INS_FROM_SCRIPTS {
        assert_eq!(row.origin, RowOrigin::FromScripts, "{}", row.scene);
        assert!(row.scene.starts_with(row.pack), "{}", row.scene);
        assert!(row.scene.ends_with("(from the scripts)"), "{}", row.scene);
    }
}

#[test]
fn a_hand_made_row_wins_over_the_rows_from_the_scripts_it_covers() {
    for row in companion_off_fights_from_scripts_in_use() {
        for hand in &COMPANION_OFF_FIGHTS {
            assert!(
                !row.start_functions
                    .iter()
                    .any(|name| hand.start_functions.contains(name)),
                "{} is covered by {}",
                row.name,
                hand.name
            );
        }
    }
    let fight = companion_off_fight_opened_by("EID_1050_a0020_Start", COPY_PACK, true, |_| true)
        .expect("the hand-made row opens in the copy pack");
    assert_eq!(fight.origin, RowOrigin::HandMade);
    for hand in &SCENE_STAND_INS {
        assert!(scene_stand_ins_from_scripts_in_use().all(|row| row.talk_id != hand.talk_id));
        let row = scene_stand_in_opened_by(hand.talk_id, Some(hand.pack), true).expect("listed");
        assert_eq!(row.origin, RowOrigin::HandMade);
    }
}

#[test]
fn a_row_from_the_scripts_opens_only_in_its_pack_with_its_on_flag() {
    let flag_on = |id| id == TEST_ON_FLAG;
    let row = fight_from_scripts_opened_by(&TEST_FIGHTS, TEST_START, TEST_PACK, flag_on)
        .expect("opens in its pack with its flag on");
    assert_eq!(row.name, TEST_FIGHTS[0].name);
    assert!(fight_from_scripts_opened_by(&TEST_FIGHTS, TEST_START, COPY_PACK, flag_on).is_none());
    assert!(fight_from_scripts_opened_by(&TEST_FIGHTS, TEST_START, TEST_PACK, |_| false).is_none());
    assert!(
        fight_from_scripts_opened_by(&TEST_FIGHTS, "EID_9999_a0020_Start", TEST_PACK, flag_on)
            .is_none()
    );
}

#[test]
fn the_rows_from_the_scripts_are_off_in_a_new_rule_and_switch_on_and_off() {
    let mut rule = CompanionOff::new();
    assert!(!rule.uses_rows_from_scripts());
    assert_eq!(rule.use_rows_from_scripts(true), None);
    assert!(rule.uses_rows_from_scripts());
    rule.enter_pack(COPY_PACK);
    assert!(matches!(
        rule.function_ran_in_story("EID_1050_a0020_Start", |_| true),
        Some(CompanionOffEvent::Opened {
            fight: "boss fight 6",
            ..
        })
    ));
    assert_eq!(rule.use_rows_from_scripts(false), None);
    assert_eq!(rule.open_fight(), Some("boss fight 6"));
    for pack in COMPANION_OFF_FIGHTS_FROM_SCRIPTS
        .iter()
        .map(|row| row.approach.pack)
    {
        assert!(
            companion_off_fights_approached(pack, false)
                .all(|row| row.origin == RowOrigin::HandMade)
        );
    }
}

#[test]
fn a_scene_row_from_the_scripts_opens_only_in_its_pack() {
    let row = scene_row_in_use();
    let mut rule = scene_rule_from_scripts_in(row.pack);
    rule.partner_talk_line(row.talk_id, 1_000);
    let opened = rule.step(&guest_in(row.pack, 1_016));
    assert!(matches!(opened[0], SceneStandInEvent::Opened { .. }));
    assert_eq!(places(&opened), 1);

    let mut elsewhere = scene_rule_from_scripts_in(COPY_PACK);
    elsewhere.partner_talk_line(row.talk_id, 1_000);
    assert!(elsewhere.step(&guest_in(COPY_PACK, 1_016)).is_empty());
    assert!(elsewhere.open_scene().is_none());

    let mut switched_off = GuestSceneStandIn::new();
    assert!(switched_off.step(&guest_in(row.pack, 0)).is_empty());
    switched_off.partner_talk_line(row.talk_id, 1_000);
    assert!(switched_off.open_scene().is_none());
}

#[test]
fn a_scene_row_from_the_scripts_with_its_closing_flag_on_places_nobody() {
    let row = scene_row_in_use();
    let mut rule = scene_rule_from_scripts_in(row.pack);
    rule.partner_talk_line(row.talk_id, 1_000);
    let closed = SceneStandInFacts {
        closing_flag_on: true,
        ..guest_in(row.pack, 1_016)
    };
    let events = rule.step(&closed);
    assert_eq!(places(&events), 0);
    assert_eq!(
        release_reason(&events),
        Some("the host's closing flag is on")
    );
}

#[test]
fn a_scene_row_from_the_scripts_is_let_go_outside_its_pack_or_when_switched_off() {
    let row = scene_row_in_use();
    let mut rule = scene_rule_from_scripts_in(row.pack);
    rule.partner_talk_line(row.talk_id, 1_000);
    assert_eq!(places(&rule.step(&guest_in(row.pack, 1_016))), 1);
    let left = rule.step(&guest_in(COPY_PACK, 2_000));
    assert_eq!(release_reason(&left), Some("this game left the row's pack"));

    let mut rule = scene_rule_from_scripts_in(row.pack);
    rule.partner_talk_line(row.talk_id, 1_000);
    rule.use_rows_from_scripts(false);
    let events = rule.step(&guest_in(row.pack, 1_016));
    assert_eq!(places(&events), 0);
    assert_eq!(
        release_reason(&events),
        Some("the rows from the scripts were switched off")
    );
}

#[test]
fn the_fights_excluded_in_the_script_make_no_row_and_two_rows_fewer_act() {
    for excluded in [
        "A_CLIFF_VILLAGE_01 EID_1000 boss 1022 (from the scripts)",
        "A_TRIAL_01 EID_1000 boss 1022 (from the scripts)",
    ] {
        assert!(
            COMPANION_OFF_FIGHTS_FROM_SCRIPTS
                .iter()
                .all(|row| row.name != excluded)
        );
    }
    assert_eq!(companion_off_fights_from_scripts_in_use().count(), 22);
}
