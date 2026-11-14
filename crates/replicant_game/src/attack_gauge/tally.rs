use crate::attack_gauge::ATTACK_GAUGE_TALLY_INTERVAL_MS;

/// Why a swing on the gauge did not lower it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GaugeSwingMiss {
    OtherDummy,
    GameTookNothing,
    SameSwing,
    NotConnected,
    DroppedByHost,
}

/// Counts for one open gauge on one side, written out once a second: hits sent (guest) or
/// received (host), hits applied, held until the host's opening arrives, and swings that did not
/// count with the last reason.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GaugeTally {
    pub sent: u32,
    pub applied: u32,
    pub held: u32,
    pub not_counted: u32,
    pub last_miss: Option<GaugeSwingMiss>,
    last_line_ms: Option<u64>,
}

impl GaugeTally {
    pub const fn new() -> Self {
        Self {
            sent: 0,
            applied: 0,
            held: 0,
            not_counted: 0,
            last_miss: None,
            last_line_ms: None,
        }
    }

    pub fn miss(&mut self, why: GaugeSwingMiss) {
        self.not_counted = self.not_counted.saturating_add(1);
        self.last_miss = Some(why);
    }

    /// True at most once per tally interval, the first time at once.
    pub fn line_due(&mut self, now_ms: u64) -> bool {
        let due = self
            .last_line_ms
            .is_none_or(|last| now_ms.saturating_sub(last) >= ATTACK_GAUGE_TALLY_INTERVAL_MS);
        if due {
            self.last_line_ms = Some(now_ms);
        }
        due
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}
