/// How a motion spends its time: evenly, or quick at first and settling at the end. Neither overshoots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Easing {
    Linear,
    EaseOut,
}

impl Easing {
    /// The eased share of a motion `t` of the way through; `t` is clamped to 0..=1, and the ends are exact.
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
        }
    }
}
