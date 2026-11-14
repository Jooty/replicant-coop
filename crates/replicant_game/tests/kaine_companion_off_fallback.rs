use replicant_game::enemies::boss_bar_state_is_open;
use replicant_game::kaine::{
    COMPANION_OFF_BOSS_GONE_FOR_GOOD_MS, COMPANION_OFF_BOSS_NEVER_CAME_MS, COMPANION_OFF_FIGHTS,
    COMPANION_OFF_FIGHTS_FROM_SCRIPTS, COMPANION_OFF_GAUGE_CLOSED_FOR_GOOD_MS, CompanionOff,
    CompanionOffBossSigns, CompanionOffEvent, CompanionOffFight, companion_off_fight_opened_by,
    companion_off_fights_from_scripts_in_use,
};
use replicant_proto::ENEMY_GROUP_COUNT;

const ELSEWHERE: &str = "TEST_PACK_02";
const TICK_MS: u64 = 500;

fn on_flag_of(row: &CompanionOffFight) -> impl Fn(u16) -> bool + use<> {
    let on = row.approach.flags.map(|flags| flags.on_game_flag);
    move |id| Some(id) == on
}

fn approach_on(row: &CompanionOffFight) -> impl Fn(u16) -> bool + use<> {
    let flags = row.approach.flags;
    move |id| flags.is_some_and(|flags| flags.on_game_flag == id)
}

fn row_the_rule_opens(
    wanted: impl Fn(&CompanionOffFight) -> bool,
) -> (&'static CompanionOffFight, &'static str) {
    companion_off_fights_from_scripts_in_use()
        .filter(|row| wanted(row))
        .find_map(|row| {
            let start = row.start_functions[0];
            let opened =
                companion_off_fight_opened_by(start, row.approach.pack, true, on_flag_of(row))?;
            (opened.name == row.name).then_some((row, start))
        })
        .expect("a generated row of this kind that its first start function opens")
}

fn unreadable_end_row() -> (&'static CompanionOffFight, &'static str) {
    row_the_rule_opens(|row| row.end_functions.is_empty())
}

fn opened_rule(row: &'static CompanionOffFight, start: &str) -> CompanionOff {
    let mut rule = CompanionOff::new();
    rule.use_rows_from_scripts(true);
    rule.enter_pack(row.approach.pack);
    let opened = rule.function_ran_in_story(start, on_flag_of(row));
    assert!(
        matches!(opened, Some(CompanionOffEvent::Opened { fight, .. }) if fight == row.name),
        "{opened:?}"
    );
    rule
}

fn signs(gauge_open: bool, boss_actors: i32, now_ms: u64) -> CompanionOffBossSigns {
    CompanionOffBossSigns {
        gauge_open: Some(gauge_open),
        boss_actors: Some(boss_actors),
        event_scene: false,
        now_ms,
    }
}

fn run_until_closed(
    rule: &mut CompanionOff,
    from_ms: u64,
    for_ms: u64,
    sign_at: impl Fn(u64) -> CompanionOffBossSigns,
) -> Option<(u64, String)> {
    let mut now = from_ms;
    while now <= from_ms + for_ms {
        if let Some(CompanionOffEvent::Closed { by, .. }) = rule.boss_signs(sign_at(now)) {
            return Some((now, by));
        }
        now += TICK_MS;
    }
    None
}

#[test]
fn every_generated_row_has_a_boss_group_beside_its_pack_as_close_signs() {
    for row in &COMPANION_OFF_FIGHTS_FROM_SCRIPTS {
        let group = row
            .boss_group
            .unwrap_or_else(|| panic!("{} has no boss group", row.name));
        assert!(group < ENEMY_GROUP_COUNT, "{}", row.name);
        assert!(!row.approach.pack.is_empty(), "{}", row.name);
    }
}

#[test]
fn the_boss_life_gauge_reads_open_only_between_its_open_and_its_close() {
    assert!(!boss_bar_state_is_open(0));
    assert!(boss_bar_state_is_open(1));
    assert!(boss_bar_state_is_open(2));
    assert!(!boss_bar_state_is_open(3));
}

#[test]
fn a_row_with_an_unreadable_end_closes_when_the_boss_life_gauge_closes_for_good() {
    let (row, start) = unreadable_end_row();
    let mut rule = opened_rule(row, start);
    assert_eq!(rule.boss_signs(signs(true, 1, 0)), None);
    let closed = run_until_closed(&mut rule, TICK_MS, 60_000, |now| signs(false, 1, now));
    let (at, by) = closed.expect("closed by the gauge");
    assert!(at >= COMPANION_OFF_GAUGE_CLOSED_FOR_GOOD_MS, "{at}");
    assert!(by.contains("the boss life gauge closing for good"), "{by}");
    assert_eq!(rule.open_fight(), None);
}

#[test]
fn a_gauge_closed_for_an_event_scene_or_opened_again_does_not_close_the_window() {
    let (row, start) = unreadable_end_row();
    let mut rule = opened_rule(row, start);
    assert_eq!(rule.boss_signs(signs(true, 1, 0)), None);
    let in_a_scene = |now| CompanionOffBossSigns {
        event_scene: true,
        ..signs(false, 1, now)
    };
    assert_eq!(
        run_until_closed(&mut rule, TICK_MS, 90_000, in_a_scene),
        None
    );
    let reopened_at = 90_000 + COMPANION_OFF_GAUGE_CLOSED_FOR_GOOD_MS / 2;
    let flicker = |now| signs(now >= reopened_at, 1, now);
    assert_eq!(
        run_until_closed(
            &mut rule,
            90_000,
            COMPANION_OFF_GAUGE_CLOSED_FOR_GOOD_MS,
            flicker
        ),
        None
    );
    assert_eq!(rule.open_fight(), Some(row.name));
}

#[test]
fn a_row_with_an_unreadable_end_closes_when_the_boss_group_is_gone() {
    let (row, start) = unreadable_end_row();
    let mut rule = opened_rule(row, start);
    let gauge_unread = |actors, now| CompanionOffBossSigns {
        gauge_open: None,
        ..signs(false, actors, now)
    };
    assert_eq!(rule.boss_signs(gauge_unread(1, 0)), None);
    let closed = run_until_closed(&mut rule, TICK_MS, 60_000, |now| gauge_unread(0, now));
    let (at, by) = closed.expect("closed by the boss group");
    assert!(at >= COMPANION_OFF_BOSS_GONE_FOR_GOOD_MS, "{at}");
    let group = row.boss_group.expect("generated rows have a boss group");
    assert!(
        by.contains(&format!("the boss actor group {group} being gone")),
        "{by}"
    );
    assert_eq!(rule.open_fight(), None);
}

#[test]
fn a_row_with_an_unreadable_end_closes_when_no_boss_comes_after_its_entry() {
    let (row, start) = unreadable_end_row();
    let mut rule = opened_rule(row, start);
    let closed = run_until_closed(&mut rule, 0, 200_000, |now| signs(false, 0, now));
    let (at, by) = closed.expect("closed when no boss came");
    assert!(at >= COMPANION_OFF_BOSS_NEVER_CAME_MS, "{at}");
    assert!(
        by.contains("no boss life gauge and no actor in group"),
        "{by}"
    );
}

#[test]
fn a_row_with_an_unreadable_end_closes_when_the_players_leave_its_pack() {
    let (row, start) = unreadable_end_row();
    let mut rule = opened_rule(row, start);
    let closed = rule.enter_pack(ELSEWHERE);
    assert!(
        matches!(&closed, Some(CompanionOffEvent::Closed { fight, by }) if *fight == row.name && by.contains(ELSEWHERE)),
        "{closed:?}"
    );
}

#[test]
fn a_window_the_story_flags_opened_waits_for_the_fight_before_no_boss_closes_it() {
    let (row, _) = row_the_rule_opens(|row| row.approach.flags.is_some());
    let mut rule = CompanionOff::new();
    rule.use_rows_from_scripts(true);
    rule.enter_pack(row.approach.pack);
    let opened = rule.story_flags(approach_on(row));
    assert!(
        matches!(opened, Some(CompanionOffEvent::Opened { .. })),
        "{opened:?}"
    );
    assert_eq!(
        run_until_closed(&mut rule, 0, 2 * COMPANION_OFF_BOSS_NEVER_CAME_MS, |now| {
            signs(false, 0, now)
        }),
        None
    );
}

#[test]
fn a_row_a_boss_sign_closed_is_not_opened_again_by_the_story_flags_until_the_pack_changes() {
    let (row, start) = row_the_rule_opens(|row| row.approach.flags.is_some());
    let mut rule = opened_rule(row, start);
    run_until_closed(&mut rule, 0, 200_000, |now| signs(false, 0, now)).expect("closed");
    assert_eq!(rule.story_flags(approach_on(row)), None);
    rule.enter_pack(ELSEWHERE);
    rule.enter_pack(row.approach.pack);
    assert!(matches!(
        rule.story_flags(approach_on(row)),
        Some(CompanionOffEvent::Opened { .. })
    ));
}

#[test]
fn a_row_without_approach_flags_opens_on_its_entry_functions_only() {
    let (row, start) = row_the_rule_opens(|row| row.approach.flags.is_none());
    for parity in [0, 1] {
        let mut rule = CompanionOff::new();
        rule.use_rows_from_scripts(true);
        rule.enter_pack(row.approach.pack);
        rule.story_flags(|id| id % 2 == parity);
        assert_ne!(rule.open_fight(), Some(row.name));
    }
    let mut rule = CompanionOff::new();
    rule.use_rows_from_scripts(true);
    rule.enter_pack(row.approach.pack);
    assert!(matches!(
        rule.function_ran_in_story(start, |_| false),
        Some(CompanionOffEvent::Opened { fight, .. }) if fight == row.name
    ));
}

#[test]
fn the_hand_made_rows_have_no_boss_signs() {
    for hand in &COMPANION_OFF_FIGHTS {
        assert_eq!(hand.boss_group, None, "{}", hand.name);
        let mut rule = CompanionOff::new();
        rule.enter_pack(hand.approach.pack);
        rule.function_ran(hand.start_functions[0]);
        assert_eq!(rule.boss_group_watched(), None);
        assert_eq!(rule.boss_signs(signs(false, 0, 0)), None);
        assert_eq!(rule.open_fight(), Some(hand.name));
    }
}
