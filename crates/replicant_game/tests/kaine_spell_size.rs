use replicant_game::kaine::{
    SPELL_LOOK_PAIRS, SPELL_SIZE_GROUPS, SPELL_SIZE_SLIDER_MAX, SPELL_SIZE_SLIDER_MIN, SizeBasis,
    SpellLook, SpellSizeGroup, SpellSizeSliders, SwappedSpellEffects, kept_size, set_style_scale,
    spell_effect_size, spell_look, style_scale,
};

const SHOT_HANDLE: u32 = 0x2001;
const OTHER_HANDLE: u32 = 0x4002;
const SPAWN_SCALE: f32 = 1.0;
const SLOT_SCALE_SET_BY_THE_SHOT: f32 = 1.5;
const WIDENED_SCALE: f32 = 2.5;
const FULL_BRIGHTNESS: f32 = 1.0;

fn pair_of(name: &str) -> &'static replicant_game::kaine::SpellLookPair {
    let SpellLook::Hers(pair) = spell_look(name) else {
        panic!("{name} has no pair");
    };
    pair
}

fn close(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-5
}

#[test]
fn every_pair_has_a_size_that_shrinks_or_keeps_her_effect() {
    for pair in &SPELL_LOOK_PAIRS {
        assert!(pair.size > 0.0 && pair.size <= 1.0, "{}", pair.original);
    }
}

#[test]
fn every_size_is_marked_as_a_guess_until_read_from_data() {
    assert!(
        SPELL_LOOK_PAIRS
            .iter()
            .all(|pair| pair.size_basis == SizeBasis::Guess)
    );
}

#[test]
fn the_rapid_shots_seen_in_the_run_are_drawn_at_about_a_third_of_her_ball() {
    for name in ["efP_blackBulletBullet01", "efP_blackBullet1stShot01"] {
        let pair = pair_of(name);
        assert_eq!(pair.hers_id, 233);
        assert!(close(pair.size, 0.35), "{name}");
        assert_eq!(pair.size_group, SpellSizeGroup::Shots);
    }
}

#[test]
fn a_charged_shot_keeps_more_of_her_big_ball_than_a_rapid_shot_of_her_ball() {
    let charged = pair_of("efP_blackBulletChargeBullet01");
    let rapid = pair_of("efP_blackBulletBullet01");
    assert!(charged.size > rapid.size);
}

#[test]
fn each_pair_sits_in_the_group_of_what_it_draws() {
    assert_eq!(
        pair_of("efP_blackBulletHit01").size_group,
        SpellSizeGroup::Hits
    );
    assert_eq!(
        pair_of("efP_blackSpearHit01").size_group,
        SpellSizeGroup::Hits
    );
    assert_eq!(
        pair_of("efP_blackBulletMuzzle01").size_group,
        SpellSizeGroup::Shots
    );
    assert_eq!(
        pair_of("efP_blackExecution_mc01").size_group,
        SpellSizeGroup::Circles
    );
    assert_eq!(
        pair_of("efP_blackHandWave01").size_group,
        SpellSizeGroup::Circles
    );
}

#[test]
fn every_group_has_a_slider() {
    assert_eq!(
        SPELL_SIZE_GROUPS.map(SpellSizeGroup::label),
        ["shots", "hits", "circles"]
    );
}

#[test]
fn sliders_start_at_one_and_leave_the_table_size() {
    let pair = pair_of("efP_blackBulletBullet01");
    assert!(close(
        spell_effect_size(pair, &SpellSizeSliders::new()),
        pair.size
    ));
}

#[test]
fn a_slider_multiplies_only_its_own_group() {
    let mut sliders = SpellSizeSliders::new();
    sliders.set(SpellSizeGroup::Shots, 2.0);
    let shot = pair_of("efP_blackBulletBullet01");
    let hit = pair_of("efP_blackBulletHit01");
    assert!(close(spell_effect_size(shot, &sliders), shot.size * 2.0));
    assert!(close(spell_effect_size(hit, &sliders), hit.size));
}

#[test]
fn a_slider_stays_inside_its_range() {
    let mut sliders = SpellSizeSliders::new();
    sliders.set(SpellSizeGroup::Hits, 50.0);
    sliders.set(SpellSizeGroup::Circles, 0.0);
    sliders.set(SpellSizeGroup::Shots, f32::NAN);
    assert!(close(
        sliders.of(SpellSizeGroup::Hits),
        SPELL_SIZE_SLIDER_MAX
    ));
    assert!(close(
        sliders.of(SpellSizeGroup::Circles),
        SPELL_SIZE_SLIDER_MIN
    ));
    assert!(close(sliders.of(SpellSizeGroup::Shots), 1.0));
}

#[test]
fn the_slider_line_reads_back_all_three_values() {
    let mut sliders = SpellSizeSliders::new();
    sliders.set(SpellSizeGroup::Shots, 0.5);
    assert_eq!(
        sliders.line(),
        "spell look: size sliders shots x0.50, hits x1.00, circles x1.00"
    );
}

#[test]
fn a_scale_the_mod_wrote_itself_is_left_alone() {
    let written = [0.35; 4];
    assert_eq!(kept_size(written, written, 0.35), None);
}

#[test]
fn a_scale_the_game_set_after_the_spawn_takes_the_size() {
    let kept = kept_size([SLOT_SCALE_SET_BY_THE_SHOT; 4], [0.35; 4], 0.35).unwrap();
    assert!(kept.iter().all(|axis| close(*axis, 0.525)));
}

#[test]
fn an_unreadable_scale_is_never_written() {
    assert_eq!(kept_size([f32::NAN; 4], [1.0; 4], 0.35), None);
    assert_eq!(kept_size([1.0e9; 4], [1.0; 4], 0.35), None);
}

#[test]
fn a_shot_slot_keeps_its_size_through_the_game_setting_and_widening_it() {
    let mut effects = SwappedSpellEffects::default();
    let size = 0.35;
    effects.note(SHOT_HANDLE, 0, size, SPAWN_SCALE * size, FULL_BRIGHTNESS);
    let first = effects.keep_sizes(|_| Some([SLOT_SCALE_SET_BY_THE_SHOT; 4]));
    assert_eq!(first.len(), 1);
    assert!(close(first[0].1[0], SLOT_SCALE_SET_BY_THE_SHOT * size));
    let settled = effects.keep_sizes(|_| Some(first[0].1));
    assert!(settled.is_empty());
    let widened = effects.keep_sizes(|_| Some([WIDENED_SCALE; 4]));
    assert!(close(widened[0].1[0], WIDENED_SCALE * size));
}

#[test]
fn an_effect_spawned_at_its_size_needs_no_write() {
    let mut effects = SwappedSpellEffects::default();
    let size = 0.4;
    effects.note(SHOT_HANDLE, 0, size, SPAWN_SCALE * size, FULL_BRIGHTNESS);
    assert!(
        effects
            .keep_sizes(|_| Some([SPAWN_SCALE * size; 4]))
            .is_empty()
    );
}

#[test]
fn an_effect_without_an_instance_yet_is_kept_for_later() {
    let mut effects = SwappedSpellEffects::default();
    effects.note(SHOT_HANDLE, 0, 0.35, 0.35, FULL_BRIGHTNESS);
    effects.note(OTHER_HANDLE, 0, 0.6, 0.6, FULL_BRIGHTNESS);
    let writes = effects.keep_sizes(|handle| (handle == OTHER_HANDLE).then_some([2.0; 4]));
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].0, OTHER_HANDLE);
    assert_eq!(effects.len(), 2);
}

#[test]
fn the_scale_is_the_style_after_the_colour() {
    let mut style = [0.3, 0.55, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0];
    assert_eq!(style_scale(&style), [2.0; 4]);
    set_style_scale(&mut style, [0.7; 4]);
    assert_eq!(style, [0.3, 0.55, 1.0, 1.0, 0.7, 0.7, 0.7, 0.7]);
}
