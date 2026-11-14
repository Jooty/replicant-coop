use replicant_game::kaine::{
    SPELL_BRIGHTNESS_GROUPS, SPELL_BRIGHTNESS_SLIDER_MAX, SPELL_BRIGHTNESS_SLIDER_MIN,
    SPELL_LOOK_PAIRS, SpellBrightnessGroup, SpellBrightnessSliders, SpellLook, SpellLookPair,
    SwappedSpellEffects, kept_brightness, set_style_colour, spell_effect_brightness, spell_look,
    style_colour,
};

const HIT_HANDLE: u32 = 0x2001;
const OTHER_HANDLE: u32 = 0x4002;
const NO_SIZE_CHANGE: f32 = 1.0;
const SPAWN_SCALE: f32 = 1.0;
const HALF_STRENGTH: f32 = 0.5;
const COLOUR_SET_BY_THE_STYLED_CALL: [f32; 3] = [0.8, 0.4, 1.0];
const WHITE: [f32; 3] = [1.0; 3];

fn pair_of(name: &str) -> &'static SpellLookPair {
    let SpellLook::Hers(pair) = spell_look(name) else {
        panic!("{name} has no pair");
    };
    pair
}

fn close(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-5
}

#[test]
fn her_muzzle_effects_are_dimmed_as_spawns() {
    for name in ["efP_blackBulletMuzzle01", "efP_blackSpearMuzzle01"] {
        let pair = pair_of(name);
        assert_eq!(
            pair.brightness_group,
            Some(SpellBrightnessGroup::Spawns),
            "{name}"
        );
        assert!(pair.brightness < 1.0, "{name}");
    }
}

#[test]
fn her_hit_effects_are_dimmed_as_hits() {
    for name in [
        "efP_blackBulletHit01",
        "efP_blackBulletChargeHit01",
        "efP_blackSpearHit01",
        "efP_blackHandHit01",
    ] {
        let pair = pair_of(name);
        assert_eq!(
            pair.brightness_group,
            Some(SpellBrightnessGroup::Hits),
            "{name}"
        );
        assert!(pair.brightness < 1.0, "{name}");
    }
}

#[test]
fn shots_and_circles_keep_their_colour() {
    for name in [
        "efP_blackBulletBullet01",
        "efP_blackBulletChargeBullet01",
        "efP_blackExecution_mc01",
        "efP_blackHandWave01",
    ] {
        let pair = pair_of(name);
        assert_eq!(pair.brightness_group, None, "{name}");
        assert!(
            close(
                spell_effect_brightness(pair, &SpellBrightnessSliders::new()),
                1.0
            ),
            "{name}"
        );
    }
}

#[test]
fn every_strength_dims_and_never_blacks_out() {
    for pair in &SPELL_LOOK_PAIRS {
        assert!(
            pair.brightness > 0.0 && pair.brightness <= 1.0,
            "{}",
            pair.original
        );
    }
}

#[test]
fn every_brightness_group_has_a_slider() {
    assert_eq!(
        SPELL_BRIGHTNESS_GROUPS.map(SpellBrightnessGroup::label),
        ["spawn brightness", "hit brightness"]
    );
}

#[test]
fn brightness_sliders_start_at_one_and_leave_the_table_strength() {
    let pair = pair_of("efP_blackBulletHit01");
    assert!(close(
        spell_effect_brightness(pair, &SpellBrightnessSliders::new()),
        pair.brightness
    ));
}

#[test]
fn a_brightness_slider_multiplies_only_its_own_group() {
    let mut sliders = SpellBrightnessSliders::new();
    sliders.set(SpellBrightnessGroup::Hits, 0.5);
    let hit = pair_of("efP_blackBulletHit01");
    let muzzle = pair_of("efP_blackBulletMuzzle01");
    assert!(close(
        spell_effect_brightness(hit, &sliders),
        hit.brightness * 0.5
    ));
    assert!(close(
        spell_effect_brightness(muzzle, &sliders),
        muzzle.brightness
    ));
}

#[test]
fn a_brightness_slider_stays_inside_its_range() {
    let mut sliders = SpellBrightnessSliders::new();
    sliders.set(SpellBrightnessGroup::Spawns, -1.0);
    sliders.set(SpellBrightnessGroup::Hits, 50.0);
    assert!(close(
        sliders.of(SpellBrightnessGroup::Spawns),
        SPELL_BRIGHTNESS_SLIDER_MIN
    ));
    assert!(close(
        sliders.of(SpellBrightnessGroup::Hits),
        SPELL_BRIGHTNESS_SLIDER_MAX
    ));
    sliders.set(SpellBrightnessGroup::Hits, f32::NAN);
    assert!(close(
        sliders.of(SpellBrightnessGroup::Hits),
        SPELL_BRIGHTNESS_SLIDER_MAX
    ));
}

#[test]
fn the_brightness_line_reads_back_both_values() {
    let mut sliders = SpellBrightnessSliders::new();
    sliders.set(SpellBrightnessGroup::Spawns, 0.25);
    assert_eq!(
        sliders.line(),
        "spell look: brightness sliders spawns x0.25, hits x1.00"
    );
}

#[test]
fn a_colour_the_game_set_is_drawn_at_the_strength() {
    assert_eq!(
        kept_brightness(COLOUR_SET_BY_THE_STYLED_CALL, None, HALF_STRENGTH),
        Some([0.4, 0.2, 0.5])
    );
}

#[test]
fn a_colour_the_mod_wrote_itself_is_left_alone() {
    let written = [0.4, 0.2, 0.5];
    assert_eq!(kept_brightness(written, Some(written), HALF_STRENGTH), None);
}

#[test]
fn full_strength_never_writes_a_colour() {
    assert_eq!(kept_brightness(WHITE, None, 1.0), None);
}

#[test]
fn an_unreadable_colour_is_never_written() {
    assert_eq!(
        kept_brightness([f32::NAN, 1.0, 1.0], None, HALF_STRENGTH),
        None
    );
    assert_eq!(
        kept_brightness([1.0e9, 1.0, 1.0], None, HALF_STRENGTH),
        None
    );
}

#[test]
fn a_styled_hit_is_dimmed_once_per_colour_the_game_sets() {
    let mut effects = SwappedSpellEffects::default();
    effects.note(HIT_HANDLE, 0, NO_SIZE_CHANGE, SPAWN_SCALE, HALF_STRENGTH);
    let first = effects.keep_brightness(|_| Some(WHITE));
    assert_eq!(first, vec![(HIT_HANDLE, [0.5; 3])]);
    let settled = effects.keep_brightness(|_| Some(first[0].1));
    assert!(settled.is_empty());
    let recoloured = effects.keep_brightness(|_| Some(COLOUR_SET_BY_THE_STYLED_CALL));
    assert_eq!(recoloured, vec![(HIT_HANDLE, [0.4, 0.2, 0.5])]);
}

#[test]
fn an_effect_at_full_brightness_is_never_recoloured() {
    let mut effects = SwappedSpellEffects::default();
    effects.note(HIT_HANDLE, 0, NO_SIZE_CHANGE, SPAWN_SCALE, 1.0);
    assert!(effects.keep_brightness(|_| Some(WHITE)).is_empty());
}

#[test]
fn an_effect_without_an_instance_yet_is_dimmed_later() {
    let mut effects = SwappedSpellEffects::default();
    effects.note(HIT_HANDLE, 0, NO_SIZE_CHANGE, SPAWN_SCALE, HALF_STRENGTH);
    effects.note(OTHER_HANDLE, 0, NO_SIZE_CHANGE, SPAWN_SCALE, HALF_STRENGTH);
    let writes = effects.keep_brightness(|handle| (handle == OTHER_HANDLE).then_some(WHITE));
    assert_eq!(writes, vec![(OTHER_HANDLE, [0.5; 3])]);
    let later = effects.keep_brightness(|handle| {
        Some(if handle == OTHER_HANDLE {
            writes[0].1
        } else {
            WHITE
        })
    });
    assert_eq!(later, vec![(HIT_HANDLE, [0.5; 3])]);
}

#[test]
fn the_colour_is_the_first_three_style_floats_and_alpha_is_kept() {
    let mut style = [0.0; 8];
    style[3] = 0.7;
    set_style_colour(&mut style, [0.1, 0.2, 0.3]);
    assert_eq!(style_colour(&style), [0.1, 0.2, 0.3]);
    assert!(close(style[3], 0.7));
    assert!(style[4..].iter().all(|value| *value == 0.0));
}
