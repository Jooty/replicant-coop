use core::ops::Range;

use replicant_proto::BULLET_STYLE_FLOATS;

use crate::kaine::SpellLookPair;

pub const SPELL_BRIGHTNESS_SLIDER_MIN: f32 = 0.0;
pub const SPELL_BRIGHTNESS_SLIDER_MAX: f32 = 2.0;
pub const MAX_SWAPPED_SPELL_EFFECT_COLOUR: f32 = 100.0;
pub const EFFECT_STYLE_COLOUR_INDICES: Range<usize> = 0..3;
pub const SPELL_BRIGHTNESS_GROUPS: [SpellBrightnessGroup; 2] =
    [SpellBrightnessGroup::Spawns, SpellBrightnessGroup::Hits];

/// Which brightness slider multiplies a pair's colour strength: her muzzle effects, or her hits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpellBrightnessGroup {
    Spawns,
    Hits,
}

/// The owner's live multipliers on the table's colour strengths. They change only what is
/// drawn: the effect instance's colour, never a hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpellBrightnessSliders {
    spawns: f32,
    hits: f32,
}

impl SpellBrightnessGroup {
    pub fn label(self) -> &'static str {
        match self {
            Self::Spawns => "spawn brightness",
            Self::Hits => "hit brightness",
        }
    }
}

impl SpellBrightnessSliders {
    pub const fn new() -> Self {
        Self {
            spawns: 1.0,
            hits: 1.0,
        }
    }

    pub fn of(&self, group: SpellBrightnessGroup) -> f32 {
        match group {
            SpellBrightnessGroup::Spawns => self.spawns,
            SpellBrightnessGroup::Hits => self.hits,
        }
    }

    pub fn set(&mut self, group: SpellBrightnessGroup, value: f32) {
        if !value.is_finite() {
            return;
        }
        let value = value.clamp(SPELL_BRIGHTNESS_SLIDER_MIN, SPELL_BRIGHTNESS_SLIDER_MAX);
        match group {
            SpellBrightnessGroup::Spawns => self.spawns = value,
            SpellBrightnessGroup::Hits => self.hits = value,
        }
    }

    pub fn line(&self) -> String {
        format!(
            "spell look: brightness sliders spawns x{:.2}, hits x{:.2}",
            self.spawns, self.hits
        )
    }
}

impl Default for SpellBrightnessSliders {
    fn default() -> Self {
        Self::new()
    }
}

/// The factor on her effect's colour: the pair's strength times its slider; 1 for a pair in
/// no brightness group.
pub fn spell_effect_brightness(pair: &SpellLookPair, sliders: &SpellBrightnessSliders) -> f32 {
    pair.brightness_group
        .map_or(1.0, |group| pair.brightness * sliders.of(group))
}

/// The colour to write into her effect when the game drew it at `drawn` after the mod last
/// wrote `written` (None before the first write): the game's colour times the strength, once
/// per colour the game sets; None when nothing is to change.
pub fn kept_brightness(
    drawn: [f32; 3],
    written: Option<[f32; 3]>,
    strength: f32,
) -> Option<[f32; 3]> {
    let unchanged =
        written.is_some_and(|written| drawn.map(f32::to_bits) == written.map(f32::to_bits));
    let readable = drawn
        .iter()
        .all(|channel| channel.is_finite() && channel.abs() <= MAX_SWAPPED_SPELL_EFFECT_COLOUR);
    let full_strength = strength.to_bits() == 1.0_f32.to_bits();
    if unchanged || !readable || full_strength || !strength.is_finite() {
        return None;
    }
    Some(drawn.map(|channel| {
        (channel * strength).clamp(
            -MAX_SWAPPED_SPELL_EFFECT_COLOUR,
            MAX_SWAPPED_SPELL_EFFECT_COLOUR,
        )
    }))
}

pub fn style_colour(style: &[f32; BULLET_STYLE_FLOATS]) -> [f32; 3] {
    core::array::from_fn(|channel| style[EFFECT_STYLE_COLOUR_INDICES.start + channel])
}

pub fn set_style_colour(style: &mut [f32; BULLET_STYLE_FLOATS], colour: [f32; 3]) {
    style[EFFECT_STYLE_COLOUR_INDICES].copy_from_slice(&colour);
}
