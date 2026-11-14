use crate::kaine::{
    COMPANION_OFF_BOSS_GONE_FOR_GOOD_MS, COMPANION_OFF_BOSS_NEVER_CAME_MS,
    COMPANION_OFF_GAUGE_CLOSED_FOR_GOOD_MS,
};

/// A close sign that needs no knowledge of the fight; it ends a window of a row with a boss group
/// whatever its end functions say. Leaving the pack is the other one, kept by the rule itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompanionOffFallback {
    GaugeClosedForGood,
    BossGone,
    BossNeverCame,
}

/// What the game shows of the boss on one tick: the boss life gauge's state and how many actors
/// the boss's group holds (`None` when unreadable, which moves no sign).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompanionOffBossSigns {
    pub gauge_open: Option<bool>,
    pub boss_actors: Option<i32>,
    pub event_scene: bool,
    pub now_ms: u64,
}

/// Time counts outside event scenes only, so a gauge closed for a scene inside the fight, or a
/// boss taken out and put back for one, does not end the window.
#[derive(Debug, Clone, Default)]
pub(crate) struct FallbackWatch {
    last_ms: Option<u64>,
    gauge_seen_open: bool,
    gauge_closed_ms: u64,
    boss_seen: bool,
    boss_gone_ms: u64,
    entry_ran: bool,
    waited_for_boss_ms: u64,
}

impl CompanionOffFallback {
    pub fn text(self, boss_group: u16) -> String {
        match self {
            Self::GaugeClosedForGood => format!(
                "the boss life gauge closing for good ({COMPANION_OFF_GAUGE_CLOSED_FOR_GOOD_MS} ms closed outside event scenes)"
            ),
            Self::BossGone => format!(
                "the boss actor group {boss_group} being gone ({COMPANION_OFF_BOSS_GONE_FOR_GOOD_MS} ms empty outside event scenes)"
            ),
            Self::BossNeverCame => format!(
                "no boss life gauge and no actor in group {boss_group} for {COMPANION_OFF_BOSS_NEVER_CAME_MS} ms after an entry function"
            ),
        }
    }
}

impl FallbackWatch {
    pub(crate) const fn new() -> Self {
        Self {
            last_ms: None,
            gauge_seen_open: false,
            gauge_closed_ms: 0,
            boss_seen: false,
            boss_gone_ms: 0,
            entry_ran: false,
            waited_for_boss_ms: 0,
        }
    }

    pub(crate) const fn entry_function_ran(&mut self) {
        self.entry_ran = true;
    }

    pub(crate) fn observe(&mut self, signs: CompanionOffBossSigns) -> Option<CompanionOffFallback> {
        let elapsed = self
            .last_ms
            .map_or(0, |last| signs.now_ms.saturating_sub(last));
        self.last_ms = Some(signs.now_ms);
        let counted = if signs.event_scene { 0 } else { elapsed };
        match signs.gauge_open {
            Some(true) => {
                self.gauge_seen_open = true;
                self.gauge_closed_ms = 0;
            }
            Some(false) if self.gauge_seen_open => self.gauge_closed_ms += counted,
            _ => {}
        }
        match signs.boss_actors {
            Some(count) if count > 0 => {
                self.boss_seen = true;
                self.boss_gone_ms = 0;
            }
            Some(_) if self.boss_seen => self.boss_gone_ms += counted,
            _ => {}
        }
        if self.entry_ran && !self.gauge_seen_open && !self.boss_seen {
            self.waited_for_boss_ms += counted;
        }
        if self.gauge_closed_ms >= COMPANION_OFF_GAUGE_CLOSED_FOR_GOOD_MS {
            return Some(CompanionOffFallback::GaugeClosedForGood);
        }
        if self.boss_gone_ms >= COMPANION_OFF_BOSS_GONE_FOR_GOOD_MS {
            return Some(CompanionOffFallback::BossGone);
        }
        (self.waited_for_boss_ms >= COMPANION_OFF_BOSS_NEVER_CAME_MS)
            .then_some(CompanionOffFallback::BossNeverCame)
    }
}
