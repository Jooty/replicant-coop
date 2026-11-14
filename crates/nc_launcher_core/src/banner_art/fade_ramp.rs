/// One vertex column of the fade: where it sits across the ramp (0 to 1) and how opaque the cover is there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FadeStop {
    pub at: f32,
    pub alpha: f32,
}

/// How opaque the cover is at `at` across the ramp: fully at the start, eased out to clear at the end.
pub fn fade_alpha(at: f32) -> f32 {
    let t = at.clamp(0.0, 1.0);
    // Smootherstep: flat at both ends, so the ramp has no visible edge where it starts or stops.
    let eased = t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    1.0 - eased
}

pub fn fade_stops(columns: usize) -> Vec<FadeStop> {
    let last = columns.max(2) - 1;
    (0..=last)
        .map(|index| {
            let at = index as f32 / last as f32;
            FadeStop {
                at,
                alpha: fade_alpha(at),
            }
        })
        .collect()
}
