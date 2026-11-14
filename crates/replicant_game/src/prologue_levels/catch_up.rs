use crate::prologue_levels::{CATCH_UP_SETTLE_MS, LEVEL_RESEND_INTERVAL_MS, PROLOGUE_PHASES};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalLevel {
    pub level: i32,
    pub experience: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartnerLevel {
    pub level: i32,
    pub experience: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct LevelStep {
    pub send: Option<LocalLevel>,
    pub gain: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct LevelCatchUp {
    partner: Option<PartnerLevel>,
    last_sent: Option<(LocalLevel, u64)>,
    last_gain_ms: Option<u64>,
    last_local: Option<LocalLevel>,
    levels_taken_away: bool,
}

impl LevelCatchUp {
    pub fn partner_reported(&mut self, level: PartnerLevel) {
        self.partner = Some(level);
    }

    pub fn partner(&self) -> Option<PartnerLevel> {
        self.partner
    }

    pub fn step(&mut self, now_ms: u64, phase: &str, local: LocalLevel) -> LevelStep {
        let mut step = LevelStep::default();
        // Sent in every phase: the swapped body's level follows the host's everywhere.
        if self.should_send(now_ms, local) {
            self.last_sent = Some((local, now_ms));
            step.send = Some(local);
        }
        if !PROLOGUE_PHASES.contains(&phase) {
            self.last_gain_ms = None;
            self.last_local = None;
            self.levels_taken_away = false;
            return step;
        }
        // A script that takes experience away ends the prologue; the partner's report predates it.
        if self
            .last_local
            .is_some_and(|before| local.experience < before.experience)
        {
            self.levels_taken_away = true;
        }
        self.last_local = Some(local);
        if !self.levels_taken_away {
            step.gain = self.gain_now(now_ms, local);
        }
        if step.gain.is_some() {
            self.last_gain_ms = Some(now_ms);
        }
        step
    }

    fn should_send(&self, now_ms: u64, local: LocalLevel) -> bool {
        match self.last_sent {
            None => true,
            Some((sent, at)) => {
                sent != local || now_ms.saturating_sub(at) >= LEVEL_RESEND_INTERVAL_MS
            }
        }
    }

    fn gain_now(&self, now_ms: u64, local: LocalLevel) -> Option<i32> {
        let partner = self.partner?;
        let missing = partner.experience.checked_sub(local.experience)?;
        if missing <= 0 {
            return None;
        }
        let settled = self
            .last_gain_ms
            .is_none_or(|at| now_ms.saturating_sub(at) >= CATCH_UP_SETTLE_MS);
        settled.then_some(missing)
    }
}
