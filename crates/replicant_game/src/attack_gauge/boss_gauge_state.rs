use crate::attack_gauge::{
    BOSS_ATTACK_GAUGE_BREAKS_LEFT_OFFSET, BOSS_ATTACK_GAUGE_STATE_BROKEN,
    BOSS_ATTACK_GAUGE_STATE_OFFSET, BOSS_ATTACK_GAUGE_STATE_OPEN,
    BOSS_ATTACK_GAUGE_STATE_TIMED_OUT,
};
use crate::memory::GameMemory;

/// The script boss's own record of its attack gauge: the state its script's open command sets to
/// open, and the node breaks it still waits for. Its message handler takes a node break only while
/// the state is open, so a gauge its script did not open can never be broken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BossGaugeState {
    pub state: i32,
    pub breaks_left: i32,
}

impl BossGaugeState {
    pub fn read<M: GameMemory>(memory: &M, boss: u64) -> Option<Self> {
        Some(Self {
            state: memory
                .read_i32(boss + BOSS_ATTACK_GAUGE_STATE_OFFSET)
                .ok()?,
            breaks_left: memory
                .read_i32(boss + BOSS_ATTACK_GAUGE_BREAKS_LEFT_OFFSET)
                .ok()?,
        })
    }

    pub const fn takes_a_break(&self) -> bool {
        self.state == BOSS_ATTACK_GAUGE_STATE_OPEN
    }

    pub fn text(&self) -> String {
        let name = match self.state {
            BOSS_ATTACK_GAUGE_STATE_OPEN => "open",
            BOSS_ATTACK_GAUGE_STATE_BROKEN => "broken",
            BOSS_ATTACK_GAUGE_STATE_TIMED_OUT => "timed out",
            0 => "none",
            _ => "unknown",
        };
        format!(
            "boss gauge state {} ({name}), breaks left {}",
            self.state, self.breaks_left
        )
    }
}

pub fn boss_gauge_state_text(state: Option<BossGaugeState>) -> String {
    state.map_or_else(
        || "boss gauge state unreadable".to_owned(),
        |state| state.text(),
    )
}
