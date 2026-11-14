use core::ops::Range;

use replicant_proto::BULLET_STYLE_FLOATS;

use crate::kaine::SpellLookPair;

pub const SPELL_SIZE_SLIDER_MIN: f32 = 0.1;
pub const SPELL_SIZE_SLIDER_MAX: f32 = 3.0;
pub const MAX_SWAPPED_SPELL_EFFECT_SCALE: f32 = 100.0;
pub const EFFECT_STYLE_SCALE_INDICES: Range<usize> = 4..BULLET_STYLE_FLOATS;
pub const SPELL_SIZE_GROUPS: [SpellSizeGroup; 3] = [
    SpellSizeGroup::Shots,
    SpellSizeGroup::Hits,
    SpellSizeGroup::Circles,
];

/// Which of the three size sliders multiplies a pair's size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpellSizeGroup {
    Shots,
    Hits,
    Circles,
}

/// Where a pair's size came from: read from the game's data, or a first value to settle by eye.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeBasis {
    FromData,
    Guess,
}

/// The owner's live multipliers on top of the table's sizes, one per group. They change only
/// what is drawn: the spawn's scale and the effect instance's scale, never a hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpellSizeSliders {
    shots: f32,
    hits: f32,
    circles: f32,
}

impl SpellSizeGroup {
    pub fn label(self) -> &'static str {
        match self {
            Self::Shots => "shots",
            Self::Hits => "hits",
            Self::Circles => "circles",
        }
    }
}

impl SpellSizeSliders {
    pub const fn new() -> Self {
        Self {
            shots: 1.0,
            hits: 1.0,
            circles: 1.0,
        }
    }

    pub fn of(&self, group: SpellSizeGroup) -> f32 {
        match group {
            SpellSizeGroup::Shots => self.shots,
            SpellSizeGroup::Hits => self.hits,
            SpellSizeGroup::Circles => self.circles,
        }
    }

    pub fn set(&mut self, group: SpellSizeGroup, value: f32) {
        if !value.is_finite() {
            return;
        }
        let value = value.clamp(SPELL_SIZE_SLIDER_MIN, SPELL_SIZE_SLIDER_MAX);
        match group {
            SpellSizeGroup::Shots => self.shots = value,
            SpellSizeGroup::Hits => self.hits = value,
            SpellSizeGroup::Circles => self.circles = value,
        }
    }

    pub fn line(&self) -> String {
        format!(
            "spell look: size sliders shots x{:.2}, hits x{:.2}, circles x{:.2}",
            self.shots, self.hits, self.circles
        )
    }
}

impl Default for SpellSizeSliders {
    fn default() -> Self {
        Self::new()
    }
}

pub fn spell_effect_size(pair: &SpellLookPair, sliders: &SpellSizeSliders) -> f32 {
    pair.size * sliders.of(pair.size_group)
}

/// The scale to write into her effect when the game drew it at `drawn` after the mod last
/// wrote `written`: a caller that sets the scale after the spawn (a shot's slot, a widening
/// shot every frame) gets its own value times the size; None when nothing changed.
pub fn kept_size(drawn: [f32; 4], written: [f32; 4], size: f32) -> Option<[f32; 4]> {
    let unchanged = drawn.map(f32::to_bits) == written.map(f32::to_bits);
    let readable = drawn
        .iter()
        .all(|axis| axis.is_finite() && axis.abs() <= MAX_SWAPPED_SPELL_EFFECT_SCALE);
    if unchanged || !readable || !size.is_finite() {
        return None;
    }
    Some(drawn.map(|axis| {
        (axis * size).clamp(
            -MAX_SWAPPED_SPELL_EFFECT_SCALE,
            MAX_SWAPPED_SPELL_EFFECT_SCALE,
        )
    }))
}

pub fn style_scale(style: &[f32; BULLET_STYLE_FLOATS]) -> [f32; 4] {
    core::array::from_fn(|axis| style[EFFECT_STYLE_SCALE_INDICES.start + axis])
}

pub fn set_style_scale(style: &mut [f32; BULLET_STYLE_FLOATS], scale: [f32; 4]) {
    style[EFFECT_STYLE_SCALE_INDICES].copy_from_slice(&scale);
}
