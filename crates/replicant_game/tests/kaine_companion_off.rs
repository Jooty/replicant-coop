use replicant_game::kaine::{
    COMPANION_LENT_AFTER_A_CALL_MS, COMPANION_LENT_AFTER_A_SCENE_MS, COMPANION_OFF_FIGHTS,
    CompanionCallLend, CompanionLendReason, CompanionOff, CompanionOffEvent, CompanionOffStep,
    CompanionOffTick, companion_off_fight_started_by, companion_off_fights_approached_in,
};

const FIGHT_PACK: &str = "A_CLIFF_VILLAGE_01";
const APPROACH_FLAG: u16 = 374;
const FIGHT_BEGUN_FLAG: u16 = 375;
const FIGHT_6_OVER_FLAG: u16 = 386;
const FIGHT_4_APPROACH_FLAG: u16 = 246;
const FIGHT_4_BEGUN_FLAG: u16 = 247;
const FIGHT_4_DONE_FLAG: u16 = 248;

fn tick(guest_in_session: bool, active_now: bool) -> CompanionOffTick {
    CompanionOffTick {
        guest_in_session,
        active_now,
        event_scene: false,
        now_ms: 0,
    }
}

fn tick_at(now_ms: u64, active_now: bool, event_scene: bool) -> CompanionOffTick {
    CompanionOffTick {
        guest_in_session: true,
        active_now,
        event_scene,
        now_ms,
    }
}

fn rule_in_open_window() -> CompanionOff {
    let mut rule = CompanionOff::new();
    rule.enter_pack(FIGHT_PACK);
    rule.function_ran("EID_1050_a0020_Start");
    rule
}

fn rule_with_her_held_off() -> CompanionOff {
    let mut rule = rule_in_open_window();
    assert_eq!(rule.step(tick(true, true)), CompanionOffStep::Deactivate);
    assert_eq!(rule.step(tick(true, false)), CompanionOffStep::Leave);
    rule
}

#[test]
fn the_companion_is_off_while_the_fight_window_is_open() {
    let mut rule = rule_in_open_window();
    assert_eq!(rule.open_fight(), Some("boss fight 6"));
    assert_eq!(rule.step(tick(true, true)), CompanionOffStep::Deactivate);
    for _ in 0..3 {
        assert_eq!(rule.step(tick(true, false)), CompanionOffStep::Leave);
    }
    assert!(rule.holds_her());
}

#[test]
fn a_callback_that_turns_her_on_is_undone_within_a_tick() {
    let mut rule = rule_in_open_window();
    assert_eq!(rule.step(tick(true, true)), CompanionOffStep::Deactivate);
    assert_eq!(rule.function_ran("EID_1050_a0035_Start"), None);
    assert_eq!(rule.step(tick(true, true)), CompanionOffStep::Deactivate);
    let report = rule.take_report().expect("window open");
    assert_eq!(report.turned_on_by, vec!["EID_1050_a0035_Start".to_owned()]);
    assert!(
        rule.take_report()
            .expect("still open")
            .turned_on_by
            .is_empty()
    );
}

#[test]
fn she_is_given_back_when_the_window_closes() {
    let mut rule = rule_with_her_held_off();
    assert!(matches!(
        rule.function_ran("EID_1050_a0070_Start"),
        Some(CompanionOffEvent::Closed { .. })
    ));
    assert_eq!(
        rule.step(tick(true, false)),
        CompanionOffStep::GiveBack { active: true }
    );
    assert_eq!(rule.step(tick(true, false)), CompanionOffStep::Leave);
    assert!(!rule.holds_her());
}

#[test]
fn she_is_given_back_when_the_players_leave_the_area() {
    let mut rule = rule_in_open_window();
    rule.step(tick(true, true));
    assert!(matches!(
        rule.enter_pack("A_NORTH_FIELD_01"),
        Some(CompanionOffEvent::Closed { .. })
    ));
    assert_eq!(
        rule.step(tick(true, false)),
        CompanionOffStep::GiveBack { active: true }
    );
}

#[test]
fn a_window_closed_after_the_game_turned_her_on_again_leaves_her_to_the_game() {
    let mut rule = rule_in_open_window();
    rule.step(tick(true, true));
    rule.enter_pack("A_NORTH_FIELD_01");
    assert_eq!(rule.step(tick(true, true)), CompanionOffStep::Leave);
}

#[test]
fn she_stays_off_when_the_scripts_left_her_off() {
    let mut rule = rule_in_open_window();
    assert_eq!(rule.step(tick(true, false)), CompanionOffStep::Leave);
    rule.function_ran("EID_1050_a0070_Start");
    assert_eq!(rule.step(tick(true, false)), CompanionOffStep::Leave);
}

#[test]
fn no_guest_means_the_rule_does_nothing() {
    let mut rule = rule_in_open_window();
    for active in [true, false, true] {
        assert_eq!(rule.step(tick(false, active)), CompanionOffStep::Leave);
    }
    assert!(!rule.holds_her());
    assert_eq!(
        rule.function_addresses_her("EID_1050_a0020_End", tick(false, false)),
        None
    );
}

#[test]
fn a_guest_who_leaves_mid_fight_gets_her_back_for_the_host() {
    let mut rule = rule_in_open_window();
    rule.step(tick(true, true));
    assert_eq!(
        rule.step(tick(false, false)),
        CompanionOffStep::GiveBack { active: true }
    );
}

#[test]
fn a_retry_entry_opens_the_window_again() {
    let mut rule = rule_in_open_window();
    rule.function_ran("EID_1050_a0070_Start");
    assert_eq!(rule.open_fight(), None);
    assert!(matches!(
        rule.function_ran("EID_1050_a0045g_Continue"),
        Some(CompanionOffEvent::Opened { .. })
    ));
}

#[test]
fn the_approach_opens_the_window_from_the_area_start_by_the_story_flags() {
    let mut rule = CompanionOff::new();
    rule.enter_pack(FIGHT_PACK);
    assert!(rule.story_flags_are_read_here());
    let opened = rule.story_flags(|id| id == APPROACH_FLAG);
    assert!(
        matches!(&opened, Some(CompanionOffEvent::Opened { fight: "boss fight 6", by }) if by.contains("g374 on, g375 off")),
        "{opened:?}"
    );
    assert_eq!(rule.step(tick(true, true)), CompanionOffStep::Deactivate);
    for name in [
        "EID_1050_a0010_010_Start",
        "EID_1050_a0010_030_End",
        "EID_1050_a0020_Start",
    ] {
        assert_eq!(rule.function_ran(name), None, "{name}");
    }
    assert_eq!(rule.open_fight(), Some("boss fight 6"));
    assert!(rule.story_flags_are_read_here());
    assert_eq!(rule.story_flags(|id| id == FIGHT_BEGUN_FLAG), None);
    assert_eq!(rule.open_fight(), Some("boss fight 6"));
}

#[test]
fn the_approach_flags_open_nothing_once_the_fight_has_begun_or_elsewhere() {
    let mut rule = CompanionOff::new();
    rule.enter_pack(FIGHT_PACK);
    assert_eq!(
        rule.story_flags(|id| id == APPROACH_FLAG || id == FIGHT_BEGUN_FLAG),
        None
    );
    assert_eq!(rule.story_flags(|_| false), None);
    rule.enter_pack("A_NORTH_FIELD_01");
    assert!(!rule.story_flags_are_read_here());
    assert_eq!(rule.story_flags(|id| id == APPROACH_FLAG), None);
    assert_eq!(rule.step(tick(true, true)), CompanionOffStep::Leave);
}

#[test]
fn a_boss_down_closes_an_approach_window_and_the_flags_do_not_reopen_it() {
    let mut rule = CompanionOff::new();
    rule.enter_pack(FIGHT_PACK);
    rule.story_flags(|id| id == APPROACH_FLAG);
    rule.function_ran("EID_1050_a0070_Start");
    assert_eq!(
        rule.story_flags(|id| id == APPROACH_FLAG || id == FIGHT_BEGUN_FLAG),
        None
    );
    assert_eq!(rule.open_fight(), None);
}

#[test]
fn a_story_jump_past_the_fight_in_its_own_pack_closes_the_window_by_the_over_flag() {
    let mut rule = rule_with_her_held_off();
    assert!(rule.story_flags_are_read_here());
    let closed = rule.story_flags(|id| id == FIGHT_BEGUN_FLAG || id == FIGHT_6_OVER_FLAG);
    assert!(
        matches!(&closed, Some(CompanionOffEvent::Closed { fight: "boss fight 6", by }) if by.contains("g386 on")),
        "{closed:?}"
    );
    assert_eq!(rule.open_fight(), None);
    assert_eq!(
        rule.step(tick(true, false)),
        CompanionOffStep::GiveBack { active: true }
    );
}

#[test]
fn the_approach_flags_open_nothing_once_the_fight_is_over() {
    let mut rule = CompanionOff::new();
    rule.enter_pack(FIGHT_PACK);
    assert_eq!(
        rule.story_flags(|id| id == APPROACH_FLAG || id == FIGHT_6_OVER_FLAG),
        None
    );
    assert_eq!(
        rule.story_flags(|id| id == FIGHT_4_APPROACH_FLAG || id == FIGHT_4_DONE_FLAG),
        None
    );
    assert_eq!(rule.open_fight(), None);
}

#[test]
fn every_fight_in_the_table_has_an_over_flag_past_its_approach() {
    for fight in &COMPANION_OFF_FIGHTS {
        let off = fight.approach.flags.map_or(0, |flags| flags.off_game_flag);
        assert!(
            fight.over_game_flag().is_some_and(|over| over > off),
            "{}",
            fight.name
        );
    }
}

#[test]
fn she_is_given_back_for_an_event_scene_and_a_moment_after_it() {
    let mut rule = rule_with_her_held_off();
    assert_eq!(
        rule.step(tick_at(1_000, false, true)),
        CompanionOffStep::Lend {
            write_active: true,
            why: CompanionLendReason::Scene
        }
    );
    assert_eq!(
        rule.step(tick_at(1_016, true, true)),
        CompanionOffStep::Leave
    );
    assert_eq!(rule.lent_for(), Some(&CompanionLendReason::Scene));
    let scene_end = 30_000;
    assert_eq!(
        rule.step(tick_at(scene_end, true, false)),
        CompanionOffStep::Leave
    );
    assert_eq!(
        rule.step(tick_at(
            scene_end + COMPANION_LENT_AFTER_A_SCENE_MS - 1,
            true,
            false
        )),
        CompanionOffStep::Leave
    );
    assert_eq!(
        rule.step(tick_at(
            scene_end + COMPANION_LENT_AFTER_A_SCENE_MS,
            true,
            false
        )),
        CompanionOffStep::LendOver {
            deactivate: true,
            why: CompanionLendReason::Scene
        }
    );
    assert_eq!(rule.lent_for(), None);
    assert_eq!(
        rule.step(tick_at(
            scene_end + COMPANION_LENT_AFTER_A_SCENE_MS + 16,
            false,
            false
        )),
        CompanionOffStep::Leave
    );
}

#[test]
fn a_call_that_addresses_her_gives_her_back_before_it_runs() {
    let mut rule = rule_with_her_held_off();
    rule.function_ran("EID_1050_a0020_End");
    assert_eq!(
        rule.function_addresses_her("EID_1050_a0020_End", tick_at(5_000, false, false)),
        Some(CompanionCallLend {
            fight: "boss fight 6",
            write_active: true
        })
    );
    assert_eq!(
        rule.lent_for(),
        Some(&CompanionLendReason::Call("EID_1050_a0020_End".to_owned()))
    );
    assert_eq!(
        rule.step(tick_at(5_016, true, false)),
        CompanionOffStep::Leave
    );
    rule.function_ran("EID_1050_a0025_Start");
    rule.function_addresses_her("EID_1050_a0025_Start", tick_at(5_020, true, false));
    assert_eq!(
        rule.step(tick_at(
            5_020 + COMPANION_LENT_AFTER_A_CALL_MS - 1,
            true,
            false
        )),
        CompanionOffStep::Leave
    );
    assert_eq!(
        rule.step(tick_at(5_020 + COMPANION_LENT_AFTER_A_CALL_MS, true, false)),
        CompanionOffStep::LendOver {
            deactivate: true,
            why: CompanionLendReason::Call("EID_1050_a0025_Start".to_owned())
        }
    );
    assert!(rule.holds_her());
}

#[test]
fn a_call_that_does_not_address_her_lends_nothing() {
    let mut rule = rule_with_her_held_off();
    rule.function_ran("BGM_EID_1050_a0020_02");
    assert_eq!(
        rule.function_addresses_her("BGM_EID_1050_a0020_02", tick_at(5_000, false, false)),
        None
    );
    assert_eq!(rule.lent_for(), None);
}

#[test]
fn a_lend_keeps_her_as_the_scripts_leave_her() {
    let mut rule = rule_with_her_held_off();
    rule.step(tick_at(1_000, false, true));
    rule.step(tick_at(1_016, true, true));
    rule.step(tick_at(1_032, false, true));
    rule.step(tick_at(2_000, false, false));
    assert_eq!(
        rule.step(tick_at(
            2_000 + COMPANION_LENT_AFTER_A_SCENE_MS,
            false,
            false
        )),
        CompanionOffStep::LendOver {
            deactivate: false,
            why: CompanionLendReason::Scene
        }
    );
    rule.function_ran("EID_1050_a0070_Start");
    assert_eq!(
        rule.step(tick_at(9_000, false, false)),
        CompanionOffStep::GiveBack { active: false }
    );
}

#[test]
fn the_fight_table_has_boss_fight_6_with_every_retry_entry() {
    assert_eq!(COMPANION_OFF_FIGHTS.len(), 2);
    for name in [
        "EID_1050_a0020_Continue",
        "EID_1050_a0025g_Continue",
        "EID_1050_a0030_Continue",
        "EID_1050_a0035g_01_Continue",
        "EID_1050_a0035g_02_Continue",
        "EID_1050_a0045g_Continue",
        "EID_1050_a0050_Continue",
        "EID_1050_a0055g_01_Continue",
        "EID_1050_a0055g_02_Continue",
        "EID_1050_a0065g_Continue",
    ] {
        let fight = companion_off_fight_started_by(name).expect(name);
        assert_eq!(fight.name, "boss fight 6", "{name}");
        assert!(fight.functions_that_address_her.contains(&name), "{name}");
    }
    assert_eq!(companion_off_fight_started_by("EID_1050_a0070_Start"), None);
    assert!(
        companion_off_fights_approached_in(FIGHT_PACK).any(|fight| fight.name == "boss fight 6")
    );
}

#[test]
fn the_fight_table_has_boss_fight_4_with_its_entry_and_every_retry_entry() {
    for name in [
        "EID_1000_a0050_01_Start",
        "EID_1000_a0050_Continue",
        "EID_1000_a0055g_Continue",
        "EID_1000_a0055_01_Start",
        "EID_1000_a0065g_01_Start",
    ] {
        let fight = companion_off_fight_started_by(name).expect(name);
        assert_eq!(fight.name, "boss fight 4", "{name}");
    }
    for name in [
        "EID_1000_a0045_01_End",
        "EID_1000_a0065g_01_End",
        "EID_1000_a0075_01_Start",
    ] {
        assert_eq!(companion_off_fight_started_by(name), None, "{name}");
    }
    let names: Vec<&str> = companion_off_fights_approached_in(FIGHT_PACK)
        .map(|fight| fight.name)
        .collect();
    assert_eq!(names, ["boss fight 6", "boss fight 4"]);
}

#[test]
fn the_companion_is_off_in_boss_fight_4_and_given_back_when_the_boss_is_down() {
    let mut rule = CompanionOff::new();
    rule.enter_pack(FIGHT_PACK);
    assert!(matches!(
        rule.function_ran("EID_1000_a0050_01_Start"),
        Some(CompanionOffEvent::Opened {
            fight: "boss fight 4",
            ..
        })
    ));
    assert_eq!(rule.step(tick(true, true)), CompanionOffStep::Deactivate);
    assert_eq!(rule.step(tick(true, false)), CompanionOffStep::Leave);
    for name in ["EID_1000_a0065g_01_Start", "Talk_Text_16", "Talk_Text_13"] {
        assert_eq!(rule.function_ran(name), None, "{name}");
    }
    assert_eq!(rule.open_fight(), Some("boss fight 4"));
    assert!(matches!(
        rule.function_ran("EID_1000_a0065g_01_End"),
        Some(CompanionOffEvent::Closed {
            fight: "boss fight 4",
            ..
        })
    ));
    assert_eq!(
        rule.step(tick(true, false)),
        CompanionOffStep::GiveBack { active: true }
    );
}

#[test]
fn a_boss_fight_4_call_that_places_her_is_lent_her_active() {
    let mut rule = CompanionOff::new();
    rule.enter_pack(FIGHT_PACK);
    assert!(matches!(
        rule.function_ran("EID_1000_a0055g_Continue"),
        Some(CompanionOffEvent::Opened {
            fight: "boss fight 4",
            ..
        })
    ));
    assert_eq!(
        rule.function_addresses_her("EID_1000_a0055g_Continue", tick_at(0, true, false)),
        Some(CompanionCallLend {
            fight: "boss fight 4",
            write_active: true,
        })
    );
    rule.step(tick_at(10, true, false));
    rule.function_ran("PUT_KAINE");
    assert!(
        rule.function_addresses_her("PUT_KAINE", tick_at(20, true, false))
            .is_some()
    );
    assert_eq!(
        rule.function_addresses_her("Talk_Text_16", tick_at(30, true, false)),
        None
    );
    let over_at = 20 + COMPANION_LENT_AFTER_A_CALL_MS;
    assert!(matches!(
        rule.step(tick_at(over_at, true, false)),
        CompanionOffStep::LendOver {
            deactivate: true,
            ..
        }
    ));
}

#[test]
fn the_boss_fight_4_approach_flags_open_its_window_and_not_after_the_fight_began() {
    let mut rule = CompanionOff::new();
    rule.enter_pack(FIGHT_PACK);
    let opened = rule.story_flags(|id| id == FIGHT_4_APPROACH_FLAG);
    assert!(
        matches!(&opened, Some(CompanionOffEvent::Opened { fight: "boss fight 4", by }) if by.contains("g246 on, g247 off")),
        "{opened:?}"
    );
    rule.function_ran("EID_1000_a0065g_01_End");
    assert_eq!(rule.open_fight(), None);
    let after_the_boss =
        |id| id == FIGHT_4_APPROACH_FLAG || id == FIGHT_4_BEGUN_FLAG || id == FIGHT_4_DONE_FLAG;
    assert_eq!(rule.story_flags(after_the_boss), None);
    assert_eq!(
        rule.story_flags(|id| id == FIGHT_4_APPROACH_FLAG || id == FIGHT_4_BEGUN_FLAG),
        None
    );
}

#[test]
fn the_boss_fight_6_approach_still_opens_boss_fight_6_late_in_the_story() {
    let mut rule = CompanionOff::new();
    rule.enter_pack(FIGHT_PACK);
    let later_story = |id| {
        id == FIGHT_4_APPROACH_FLAG
            || id == FIGHT_4_BEGUN_FLAG
            || id == FIGHT_4_DONE_FLAG
            || id == APPROACH_FLAG
    };
    assert!(matches!(
        rule.story_flags(later_story),
        Some(CompanionOffEvent::Opened {
            fight: "boss fight 6",
            ..
        })
    ));
}

#[test]
fn no_guest_means_the_boss_fight_4_rule_does_nothing() {
    let mut rule = CompanionOff::new();
    rule.enter_pack(FIGHT_PACK);
    rule.function_ran("EID_1000_a0055_01_Start");
    assert_eq!(rule.step(tick(false, true)), CompanionOffStep::Leave);
    assert_eq!(
        rule.function_addresses_her("EID_1000_a0055_01_Start", tick(false, true)),
        None
    );
}
