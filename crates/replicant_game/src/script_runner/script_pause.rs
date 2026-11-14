use crate::script_runner::SCRIPT_PAUSE_STALE_AFTER_MS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScriptPauseWatch {
    open_pauses: u32,
    last_pause_on_ms: u64,
}

impl ScriptPauseWatch {
    pub const fn new() -> Self {
        Self {
            open_pauses: 0,
            last_pause_on_ms: 0,
        }
    }

    pub fn pause_on_called(&mut self, now_ms: u64) {
        self.open_pauses = self.open_pauses.saturating_add(1);
        self.last_pause_on_ms = now_ms;
    }

    /// The game's `_PauseOff` clears the whole pause mask however many `_PauseOn` came before it
    /// (every logged call goes to 0x0), so one ends every open script pause.
    pub fn pause_off_called(&mut self) {
        self.open_pauses = 0;
    }

    pub fn in_force(&self, now_ms: u64) -> bool {
        self.open_pauses > 0
            && now_ms.saturating_sub(self.last_pause_on_ms) < SCRIPT_PAUSE_STALE_AFTER_MS
    }
}
