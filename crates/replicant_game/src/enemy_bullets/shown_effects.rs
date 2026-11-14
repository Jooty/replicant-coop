use std::collections::BTreeMap;

use replicant_proto::BULLET_TRANSFORM_FLOATS;

use crate::enemy_bullets::{MAX_SHOWN_BULLET_EFFECTS_LOGGED, longest_axis};

#[derive(Debug, Clone, Copy, Default)]
struct ShownEffect {
    started: u32,
    moves: u32,
    longest_axis: f32,
}

/// The guest's count of the effect-drawn host bullets it showed in one summary window, by the
/// first effect id of their look, with the longest axis each was drawn with (a beam's stretch).
#[derive(Debug, Clone, Default)]
pub struct ShownEffectTally {
    by_effect: BTreeMap<i32, ShownEffect>,
}

impl ShownEffectTally {
    pub fn note_start(&mut self, effect_id: i32, transform: &[f32; BULLET_TRANSFORM_FLOATS]) {
        let shown = self.by_effect.entry(effect_id).or_default();
        shown.started += 1;
        shown.longest_axis = shown.longest_axis.max(longest_axis(transform));
    }

    pub fn note_move(&mut self, effect_id: i32, transform: &[f32; BULLET_TRANSFORM_FLOATS]) {
        let shown = self.by_effect.entry(effect_id).or_default();
        shown.moves += 1;
        shown.longest_axis = shown.longest_axis.max(longest_axis(transform));
    }

    /// The summary of the window that ends now, if any effect-drawn bullet was shown or moved.
    pub fn take_summary(&mut self) -> Option<String> {
        let by_effect = core::mem::take(&mut self.by_effect);
        if by_effect.is_empty() {
            return None;
        }
        let parts: Vec<String> = by_effect
            .iter()
            .take(MAX_SHOWN_BULLET_EFFECTS_LOGGED)
            .map(|(effect_id, shown)| {
                format!(
                    "{effect_id}: {} started, {} moves, longest axis {:.1}",
                    shown.started, shown.moves, shown.longest_axis
                )
            })
            .collect();
        let more = by_effect
            .len()
            .saturating_sub(MAX_SHOWN_BULLET_EFFECTS_LOGGED);
        let more_text = if more > 0 {
            format!(" and {more} more")
        } else {
            String::new()
        };
        Some(format!(
            "enemy bullets: last 5 s shown by effect id: {}{more_text}",
            parts.join("; ")
        ))
    }
}
