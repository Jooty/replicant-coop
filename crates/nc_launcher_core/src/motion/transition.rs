use crate::motion::Easing;

/// A value on its way to a target, started at a moment of the launcher's clock (seconds).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    from: f32,
    to: f32,
    started_at: f64,
    seconds: f32,
    easing: Easing,
}

impl Transition {
    pub fn at_rest(value: f32) -> Self {
        Self {
            from: value,
            to: value,
            started_at: 0.0,
            seconds: 0.0,
            easing: Easing::Linear,
        }
    }

    pub fn target(&self) -> f32 {
        self.to
    }

    pub fn rising(&self) -> bool {
        self.to >= self.from
    }

    pub fn value_at(&self, now: f64) -> f32 {
        let t = self.share_done(now);
        if t >= 1.0 {
            return self.to;
        }
        self.from + (self.to - self.from) * self.easing.apply(t)
    }

    pub fn is_running(&self, now: f64) -> bool {
        self.from != self.to && self.share_done(now) < 1.0
    }

    /// Heads for `target` from wherever the value is at `now`, so an interrupted motion never jumps. A whole
    /// run from 0 to 1 takes `full_ms` and a shorter one its share; without motion the value is there at once.
    pub fn head_for(&mut self, target: f32, now: f64, full_ms: f32, easing: Easing, motion: bool) {
        if target == self.to {
            return;
        }
        let from = self.value_at(now);
        let distance = (target - from).abs().min(1.0);
        let seconds = if motion {
            full_ms / 1000.0 * distance
        } else {
            0.0
        };
        *self = Self {
            from,
            to: target,
            started_at: now,
            seconds,
            easing,
        };
    }

    fn share_done(&self, now: f64) -> f32 {
        if self.seconds <= 0.0 {
            return 1.0;
        }
        ((now - self.started_at) as f32 / self.seconds).max(0.0)
    }
}
