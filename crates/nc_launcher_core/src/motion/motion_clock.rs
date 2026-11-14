use crate::motion::{Easing, Sweep, Transition};

/// One look at the launcher's clock while a frame is drawn: it reads every motion at `now`, cuts every
/// motion to its end state when motion is off, and keeps when the next frame is needed for what is moving.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionClock {
    now: f64,
    motion: bool,
    next_frame_in: Option<f32>,
}

impl MotionClock {
    pub fn new(now: f64, motion: bool) -> Self {
        Self {
            now,
            motion,
            next_frame_in: None,
        }
    }

    pub fn now(&self) -> f64 {
        self.now
    }

    pub fn motion(&self) -> bool {
        self.motion
    }

    /// Seconds until a frame is needed again, or `None` when nothing is moving or waiting to.
    pub fn next_frame_in(&self) -> Option<f32> {
        self.next_frame_in
    }

    pub fn sample(&mut self, transition: &Transition) -> f32 {
        if !self.motion {
            return transition.target();
        }
        if transition.is_running(self.now) {
            self.wake_in(0.0);
        }
        transition.value_at(self.now)
    }

    pub fn sweep(&mut self, transition: &Transition) -> Sweep {
        Sweep {
            amount: self.sample(transition),
            filling: transition.rising(),
        }
    }

    /// How far something that appeared at `since` has come in, `delay_ms` after that and over `ms`: 0 before,
    /// exactly 1 from the end on.
    pub fn entrance(&mut self, since: f64, delay_ms: f32, ms: f32, easing: Easing) -> f32 {
        if !self.motion {
            return 1.0;
        }
        let elapsed_ms = ((self.now - since) * 1000.0) as f32 - delay_ms;
        if elapsed_ms >= ms {
            return 1.0;
        }
        self.wake_in(0.0);
        if elapsed_ms <= 0.0 {
            return 0.0;
        }
        easing.apply(elapsed_ms / ms)
    }

    /// A slow steady drift, one whole pixel every `ms_per_px`, wrapping at `period_px`. It asks only for the
    /// frame of its next pixel, and stands still without motion.
    pub fn drift_px(&mut self, ms_per_px: f32, period_px: f32) -> f32 {
        if !self.motion {
            return 0.0;
        }
        let step_seconds = f64::from(ms_per_px) / 1000.0;
        let next_step = ((self.now / step_seconds).floor() + 1.0) * step_seconds;
        self.wake_in((next_step - self.now) as f32);
        drift_offset_px(self.now, ms_per_px, period_px)
    }

    pub fn wake_in(&mut self, seconds: f32) {
        let seconds = seconds.max(0.0);
        self.next_frame_in = Some(self.next_frame_in.map_or(seconds, |soon| soon.min(seconds)));
    }
}

/// Where a drift of one pixel every `ms_per_px` stands at `now`, wrapped at `period_px`.
pub fn drift_offset_px(now: f64, ms_per_px: f32, period_px: f32) -> f32 {
    let steps = (now * 1000.0 / f64::from(ms_per_px)).floor();
    (steps % f64::from(period_px.max(1.0))) as f32
}
