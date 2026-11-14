use crate::stall_watch::{
    STALL_LATER_SAMPLE_EVERY_MS, STALL_MAX_SAMPLES_PER_STALL, STALL_SAMPLE_AFTER_MS,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StallVerdict {
    Ticking,
    Waiting,
    /// Sample where the game thread stands: it has not ticked for `stalled_ms`.
    Sample {
        stalled_ms: u64,
        index: u32,
    },
    /// It ticks again after a stall that was sampled.
    Recovered {
        stalled_ms: u64,
        samples: u32,
    },
}

/// When to sample a game thread that stopped ticking: a few times early, then rarely, and a
/// line when it ticks again, so a long load reads as a stall and a hang as a run of samples.
#[derive(Debug, Clone, Default)]
pub struct StallWatch {
    tick_seen: Option<u64>,
    samples: u32,
}

impl StallWatch {
    pub const fn new() -> Self {
        Self {
            tick_seen: None,
            samples: 0,
        }
    }

    pub fn check(&mut self, last_tick_ms: Option<u64>, now_ms: u64) -> StallVerdict {
        let Some(last_tick_ms) = last_tick_ms else {
            return StallVerdict::Ticking;
        };
        if self.tick_seen != Some(last_tick_ms) {
            let previous = self.tick_seen.replace(last_tick_ms);
            let samples = std::mem::take(&mut self.samples);
            if samples > 0
                && let Some(previous) = previous
            {
                return StallVerdict::Recovered {
                    stalled_ms: last_tick_ms.saturating_sub(previous),
                    samples,
                };
            }
        }
        let stalled_ms = now_ms.saturating_sub(last_tick_ms);
        if self.samples >= STALL_MAX_SAMPLES_PER_STALL
            || stalled_ms < sample_due_after(self.samples)
        {
            return if self.samples == 0 {
                StallVerdict::Ticking
            } else {
                StallVerdict::Waiting
            };
        }
        let index = self.samples;
        self.samples += 1;
        StallVerdict::Sample { stalled_ms, index }
    }
}

fn sample_due_after(index: u32) -> u64 {
    let early = STALL_SAMPLE_AFTER_MS.len() as u32;
    if index < early {
        return STALL_SAMPLE_AFTER_MS[index as usize];
    }
    let last_early = STALL_SAMPLE_AFTER_MS[STALL_SAMPLE_AFTER_MS.len() - 1];
    last_early + u64::from(index - early + 1) * STALL_LATER_SAMPLE_EVERY_MS
}
