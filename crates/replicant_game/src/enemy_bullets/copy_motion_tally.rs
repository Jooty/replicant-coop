use crate::enemy_bullets::CopyCorrection;

const PERCENT: f32 = 100.0;

/// The guest's once-a-second account of shot copies moved between host messages and the
/// corrections the messages made.
#[derive(Debug, Default)]
pub struct CopyMotionTally {
    frames: u32,
    most_copies: usize,
    most_resizing: usize,
    corrections: u32,
    largest_place: f32,
    largest_scale_change: f32,
}

impl CopyMotionTally {
    pub fn note_frame(&mut self, copies: usize, resizing: usize) {
        if copies == 0 {
            return;
        }
        self.frames += 1;
        self.most_copies = self.most_copies.max(copies);
        self.most_resizing = self.most_resizing.max(resizing);
    }

    pub fn note_correction(&mut self, correction: CopyCorrection) {
        self.corrections += 1;
        self.largest_place = self.largest_place.max(correction.place);
        self.largest_scale_change = self
            .largest_scale_change
            .max((correction.scale_ratio - 1.0).abs());
    }

    /// The line for the time since the last one, or `None` when no copy was in the air.
    pub fn take_line(&mut self) -> Option<String> {
        let tally = core::mem::take(self);
        if tally.frames == 0 && tally.corrections == 0 {
            return None;
        }
        Some(format!(
            "shot copies: last 1 s moved up to {} copies on their own over {} frames (up to {} growing or resizing), {} host corrections, largest {:.2} m and {:.0}% in size",
            tally.most_copies,
            tally.frames,
            tally.most_resizing,
            tally.corrections,
            tally.largest_place,
            tally.largest_scale_change * PERCENT
        ))
    }
}
