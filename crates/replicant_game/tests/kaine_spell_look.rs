use std::collections::HashSet;

use replicant_game::kaine::{
    MAX_TRACKED_SWAPPED_SPELL_EFFECTS, PLAYER_SPELL_EFFECT_PREFIX, SPELL_LOOK_PAIRS,
    SWAPPED_BODY_SPELL_TINT_RGB, SWAPPED_SPELL_EFFECT_MAX_LIFE_MS, SpellLook, SpellLookLog,
    SwappedSpellEffects, her_effect_id, read_spell_effect_name, sent_shot_effect_ids,
    sent_spell_effect_name, spell_kind, spell_look, spell_look_line, swapped_spell_look,
    tint_style,
};

use crate::fake_memory::FakeMemory;

const NAME_ADDRESS: u64 = 0x3_6000_0000;
const HER_EFFECT_PREFIX: &str = "efC_kaine_";
const SHOT_SEEN_ON_THE_SWAPPED_BODY: [i32; 4] = [51, 196, 0, 0];
const NO_SIZE_CHANGE: f32 = 1.0;
const SPAWN_SCALE: f32 = 1.0;
const FULL_BRIGHTNESS: f32 = 1.0;

#[test]
fn every_pair_turns_a_spell_effect_into_one_of_hers() {
    for pair in &SPELL_LOOK_PAIRS {
        assert!(
            pair.original.starts_with(PLAYER_SPELL_EFFECT_PREFIX),
            "{}",
            pair.original
        );
        assert!(
            pair.her_name().starts_with(HER_EFFECT_PREFIX),
            "{}",
            pair.her_name()
        );
    }
}

#[test]
fn no_spell_effect_has_two_pairs() {
    let mut originals = HashSet::new();
    let mut ids = HashSet::new();
    for pair in &SPELL_LOOK_PAIRS {
        assert!(originals.insert(pair.original), "{}", pair.original);
        if let Some(id) = pair.original_id {
            assert!(ids.insert(id), "{id}");
        }
    }
}

#[test]
fn the_shots_seen_in_the_logs_become_her_balls() {
    assert_eq!(her_effect_id(47), Some(233));
    assert_eq!(her_effect_id(245), Some(233));
    assert_eq!(her_effect_id(51), Some(288));
    assert_eq!(her_effect_id(192), Some(588));
    assert_eq!(her_effect_id(0), None);
}

#[test]
fn a_paired_spell_effect_is_hers_on_the_swapped_body() {
    let SpellLook::Hers(pair) = swapped_spell_look("efP_blackBulletBullet01", true, false) else {
        panic!("expected her effect");
    };
    assert_eq!(pair.her_name(), "efC_kaine_MagicBall01");
    assert_eq!(pair.hers_id, 233);
}

#[test]
fn the_first_body_keeps_its_own_spells() {
    assert_eq!(
        swapped_spell_look("efP_blackBulletBullet01", false, false),
        SpellLook::AsItIs
    );
    assert_eq!(
        swapped_spell_look("efP_blackWall01", false, false),
        SpellLook::AsItIs
    );
}

#[test]
fn an_effect_played_for_the_partner_is_never_swapped() {
    assert_eq!(
        swapped_spell_look("efP_blackBulletBullet01", true, true),
        SpellLook::AsItIs
    );
}

#[test]
fn a_spell_effect_without_a_pair_takes_her_colour() {
    assert_eq!(spell_look("efP_blackWall01"), SpellLook::Tinted);
    assert_eq!(spell_look("efP_blackRotationBrade01"), SpellLook::Tinted);
}

#[test]
fn an_effect_that_is_not_a_spell_is_left_alone() {
    assert_eq!(spell_look("efP_avoidSmoke01"), SpellLook::AsItIs);
    assert_eq!(spell_look("efP_wBook_chargeLoop01"), SpellLook::AsItIs);
    assert_eq!(spell_look("efC_kaine_MagicBall01"), SpellLook::AsItIs);
}

#[test]
fn the_partner_is_sent_her_name_for_a_paired_effect() {
    assert_eq!(
        sent_spell_effect_name("efP_blackSpearBullet01".to_owned(), true),
        "efC_kaine_Trace01"
    );
    assert_eq!(
        sent_spell_effect_name("efP_blackSpearBullet01".to_owned(), false),
        "efP_blackSpearBullet01"
    );
    assert_eq!(
        sent_spell_effect_name("efP_blackWall01".to_owned(), true),
        "efP_blackWall01"
    );
}

#[test]
fn the_partner_is_sent_her_ids_for_a_shot() {
    assert_eq!(
        sent_shot_effect_ids(SHOT_SEEN_ON_THE_SWAPPED_BODY, true),
        [288, 271, 0, 0]
    );
    assert_eq!(
        sent_shot_effect_ids(SHOT_SEEN_ON_THE_SWAPPED_BODY, false),
        SHOT_SEEN_ON_THE_SWAPPED_BODY
    );
}

#[test]
fn her_colour_replaces_the_colour_and_keeps_alpha_and_intensities() {
    let mut style = [0.9, 0.1, 0.1, 0.5, 2.0, 2.0, 2.0, 2.0];
    tint_style(&mut style);
    let [red, green, blue] = SWAPPED_BODY_SPELL_TINT_RGB;
    assert_eq!(style, [red, green, blue, 0.5, 2.0, 2.0, 2.0, 2.0]);
}

#[test]
fn the_spell_kind_comes_from_the_effect_name() {
    assert_eq!(spell_kind("efP_blackBulletHit01"), Some("Bullet"));
    assert_eq!(spell_kind("efP_blackExecution_mc01"), Some("Execution"));
    assert_eq!(spell_kind("efP_blackGluttonShot02"), Some("Glutton"));
    assert_eq!(spell_kind("efP_avoidSmoke01"), None);
}

#[test]
fn the_log_line_names_both_effects_and_their_ids() {
    let line = spell_look_line(
        "efP_blackBulletBullet01",
        spell_look("efP_blackBulletBullet01"),
    );
    assert_eq!(
        line,
        "spell look: Bullet spell effect efP_blackBulletBullet01 (id 47) -> efC_kaine_MagicBall01 (id 233), same role, size x0.35 (a guess)"
    );
}

#[test]
fn the_log_line_of_a_dimmed_effect_names_its_colour_strength() {
    let line = spell_look_line(
        "efP_blackBulletMuzzle01",
        spell_look("efP_blackBulletMuzzle01"),
    );
    assert_eq!(
        line,
        "spell look: Bullet spell effect efP_blackBulletMuzzle01 (by name) -> efC_kaine_MagicBallStart02 (id 289), a guess, size x0.40 (a guess), colour x0.50 (spawn brightness, a guess)"
    );
}

#[test]
fn each_spell_effect_is_logged_once() {
    let mut log = SpellLookLog::default();
    assert!(log.is_first("efP_blackBulletBullet01"));
    assert!(!log.is_first("efP_blackBulletBullet01"));
    assert!(log.is_first("efP_blackBulletHit01"));
}

#[test]
fn her_effect_that_outlives_the_limit_is_ended() {
    let mut effects = SwappedSpellEffects::default();
    effects.note(0x2001, 1_000, NO_SIZE_CHANGE, SPAWN_SCALE, FULL_BRIGHTNESS);
    effects.note(0x4002, 5_000, NO_SIZE_CHANGE, SPAWN_SCALE, FULL_BRIGHTNESS);
    assert!(
        effects
            .take_overdue(1_000 + SWAPPED_SPELL_EFFECT_MAX_LIFE_MS - 1)
            .is_empty()
    );
    assert_eq!(
        effects.take_overdue(1_000 + SWAPPED_SPELL_EFFECT_MAX_LIFE_MS),
        vec![0x2001]
    );
    assert_eq!(effects.len(), 1);
}

#[test]
fn the_watch_list_drops_the_oldest_when_full() {
    let mut effects = SwappedSpellEffects::default();
    for handle in 0..=MAX_TRACKED_SWAPPED_SPELL_EFFECTS as u32 {
        effects.note(handle, 0, NO_SIZE_CHANGE, SPAWN_SCALE, FULL_BRIGHTNESS);
    }
    assert_eq!(effects.len(), MAX_TRACKED_SWAPPED_SPELL_EFFECTS);
    assert!(
        !effects
            .take_overdue(SWAPPED_SPELL_EFFECT_MAX_LIFE_MS)
            .contains(&0)
    );
}

#[test]
fn a_spell_effect_name_is_read_from_memory() {
    let mut memory = FakeMemory::new();
    memory.put(NAME_ADDRESS, b"efP_blackHandHit01\0padding");
    assert_eq!(
        read_spell_effect_name(&memory, NAME_ADDRESS).unwrap(),
        Some("efP_blackHandHit01".to_owned())
    );
}

#[test]
fn another_effect_name_is_not_read_past_the_prefix() {
    let mut memory = FakeMemory::new();
    memory.put(NAME_ADDRESS, b"efP_avoidS");
    assert_eq!(read_spell_effect_name(&memory, NAME_ADDRESS).unwrap(), None);
}
