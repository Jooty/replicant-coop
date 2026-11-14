use replicant_proto::BULLET_TRANSFORM_FLOATS;

use crate::bullet_flights::{FlightSample, MAX_EXTRAPOLATION_MS};
use crate::enemy_bullets::{
    MAX_SMOOTHED_SHOT_PLACE_CORRECTION_METRES, MAX_SMOOTHED_SHOT_SCALE_CORRECTION_RATIO,
    SHOT_CORRECTION_BLEND_MS,
};

const MILLIS_PER_SECOND: f32 = 1000.0;
const TRANSLATION_COLUMN: usize = 3;
const TRANSFORM_ROW_FLOATS: usize = 4;

/// What one host message changed in a copy that was already moving on its own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CopyCorrection {
    /// Metres between where the copy was drawn and where the message puts it.
    pub place: f32,
    /// The scale the copy was drawn at over the scale the message brings; 1 without a new look.
    pub scale_ratio: f32,
}

/// The guest's copy of a host shot between messages: it flies on with the last velocity and
/// grows at the last growth rate, every frame; what a message corrects is blended out over
/// `SHOT_CORRECTION_BLEND_MS` instead of jumping.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CopyMotion {
    latest: FlightSample,
    growth_per_second: f32,
    scale_base: f32,
    scale_base_ms: u64,
    place_correction: [f32; 3],
    place_corrected_ms: u64,
    scale_correction: f32,
    scale_corrected_ms: u64,
}

impl CopyMotion {
    pub fn new(latest: FlightSample, growth_per_second: f32) -> Self {
        Self {
            latest,
            growth_per_second,
            scale_base: 1.0,
            scale_base_ms: latest.received_ms,
            place_correction: [0.0; 3],
            place_corrected_ms: latest.received_ms,
            scale_correction: 1.0,
            scale_corrected_ms: latest.received_ms,
        }
    }

    /// Where to draw the copy at `now_ms`.
    pub fn transform(&self, now_ms: u64) -> [f32; BULLET_TRANSFORM_FLOATS] {
        let seconds = extrapolated_seconds(self.latest.received_ms, now_ms);
        let left = correction_left(self.place_corrected_ms, now_ms);
        let mut transform = self.latest.transform;
        for axis in 0..3 {
            transform[axis * TRANSFORM_ROW_FLOATS + TRANSLATION_COLUMN] +=
                self.latest.velocity[axis] * seconds + self.place_correction[axis] * left;
        }
        transform
    }

    /// The scale to draw the copy's effects at, as a factor of the last look the host sent.
    pub fn scale_factor(&self, now_ms: u64) -> f32 {
        let left = correction_left(self.scale_corrected_ms, now_ms);
        self.grown_scale(now_ms) * (1.0 + (self.scale_correction - 1.0) * left)
    }

    /// Whether the copy's scale changes from frame to frame now.
    pub fn resizing(&self, now_ms: u64) -> bool {
        self.growth_per_second > 0.0
            || (self.scale_correction != 1.0
                && correction_left(self.scale_corrected_ms, now_ms) > 0.0)
    }

    /// Takes the host's next message, received at `sample.received_ms`. `rescaled` is the scale
    /// of the look it brings over the scale of the look before, when it brings a new one.
    pub fn correct(
        &mut self,
        sample: FlightSample,
        growth_per_second: f32,
        rescaled: Option<f32>,
    ) -> CopyCorrection {
        let now_ms = sample.received_ms;
        let drawn = self.transform(now_ms);
        let offset: [f32; 3] = core::array::from_fn(|axis| {
            let index = axis * TRANSFORM_ROW_FLOATS + TRANSLATION_COLUMN;
            drawn[index] - sample.transform[index]
        });
        let place = offset.iter().map(|axis| axis * axis).sum::<f32>().sqrt();
        // A jump too far to glide over (a reused slot, a teleport) is taken at once.
        self.place_correction = if place <= MAX_SMOOTHED_SHOT_PLACE_CORRECTION_METRES {
            offset
        } else {
            [0.0; 3]
        };
        self.place_corrected_ms = now_ms;
        self.latest = sample;
        let mut scale_ratio = 1.0;
        match rescaled.filter(|ratio| *ratio > 0.0 && ratio.is_finite()) {
            Some(ratio) => {
                scale_ratio = self.scale_factor(now_ms) / ratio;
                let smooth = (1.0 / MAX_SMOOTHED_SHOT_SCALE_CORRECTION_RATIO
                    ..=MAX_SMOOTHED_SHOT_SCALE_CORRECTION_RATIO)
                    .contains(&scale_ratio);
                self.scale_correction = if smooth { scale_ratio } else { 1.0 };
                self.scale_corrected_ms = now_ms;
                self.scale_base = 1.0;
            }
            None => self.scale_base = self.grown_scale(now_ms),
        }
        self.scale_base_ms = now_ms;
        self.growth_per_second = growth_per_second;
        CopyCorrection { place, scale_ratio }
    }

    fn grown_scale(&self, now_ms: u64) -> f32 {
        let seconds = extrapolated_seconds(self.scale_base_ms, now_ms);
        self.scale_base * (1.0 + self.growth_per_second * seconds)
    }
}

fn extrapolated_seconds(since_ms: u64, now_ms: u64) -> f32 {
    now_ms.saturating_sub(since_ms).min(MAX_EXTRAPOLATION_MS) as f32 / MILLIS_PER_SECOND
}

fn correction_left(corrected_ms: u64, now_ms: u64) -> f32 {
    let elapsed = now_ms.saturating_sub(corrected_ms);
    if elapsed >= SHOT_CORRECTION_BLEND_MS {
        return 0.0;
    }
    1.0 - elapsed as f32 / SHOT_CORRECTION_BLEND_MS as f32
}
