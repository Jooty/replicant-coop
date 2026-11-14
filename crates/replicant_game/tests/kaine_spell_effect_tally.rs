use replicant_game::kaine::{
    HerEffectRole, SPELL_EFFECT_TALLY_INTERVAL_MS, SpellEffectTally, SpellLook, her_effect_role,
    spell_look,
};

const START_MS: u64 = 10_000;

fn role_of(name: &str) -> HerEffectRole {
    let SpellLook::Hers(pair) = spell_look(name) else {
        panic!("{name} has no pair");
    };
    her_effect_role(pair)
}

#[test]
fn each_part_of_a_cast_is_counted_in_its_own_role() {
    assert_eq!(role_of("efP_blackBulletMuzzle01"), HerEffectRole::Spawn);
    assert_eq!(role_of("efP_blackBulletBullet01"), HerEffectRole::Shot);
    assert_eq!(role_of("efP_blackBulletHit01"), HerEffectRole::Hit);
    assert_eq!(role_of("efP_blackExecution_mc01"), HerEffectRole::Circle);
}

#[test]
fn nothing_is_logged_before_the_window_has_run() {
    let mut tally = SpellEffectTally::default();
    tally.count_made(HerEffectRole::Shot, START_MS);
    assert_eq!(
        tally.take_line(START_MS + SPELL_EFFECT_TALLY_INTERVAL_MS - 1),
        None
    );
}

#[test]
fn nothing_is_logged_while_not_casting() {
    let mut tally = SpellEffectTally::default();
    assert_eq!(tally.take_line(START_MS), None);
    assert_eq!(
        tally.take_line(START_MS + 10 * SPELL_EFFECT_TALLY_INTERVAL_MS),
        None
    );
}

#[test]
fn a_second_of_casting_logs_what_each_shot_made_and_starts_again() {
    let mut tally = SpellEffectTally::default();
    for shot in 0..4 {
        tally.count_made(HerEffectRole::Spawn, START_MS + shot);
        tally.count_made(HerEffectRole::Shot, START_MS + shot);
        tally.count_shot_slot_not_resent(START_MS + shot);
    }
    tally.count_made(HerEffectRole::Hit, START_MS + 10);
    tally.count_made(HerEffectRole::Hit, START_MS + 20);
    assert_eq!(
        tally
            .take_line(START_MS + SPELL_EFFECT_TALLY_INTERVAL_MS)
            .as_deref(),
        Some(
            "spell look: last 1 s her effects made 4 spawn, 4 shot, 2 hit, 0 circle (per shot 1.00 spawn, 0.50 hit); 4 shot slot effects not sent again as world effects (the shot sync draws them)"
        )
    );
    assert_eq!(tally.made(HerEffectRole::Shot), 0);
    assert_eq!(
        tally.take_line(START_MS + 2 * SPELL_EFFECT_TALLY_INTERVAL_MS),
        None
    );
}

#[test]
fn a_cast_without_a_shot_says_so() {
    let mut tally = SpellEffectTally::default();
    tally.count_made(HerEffectRole::Circle, START_MS);
    let line = tally
        .take_line(START_MS + SPELL_EFFECT_TALLY_INTERVAL_MS)
        .unwrap_or_default();
    assert!(line.contains("1 circle (no shot)"), "{line}");
}
