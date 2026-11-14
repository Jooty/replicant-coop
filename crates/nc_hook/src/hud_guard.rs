use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::diagnostics::log_line;

const HUD_PANICKED_LINE: &str =
    "overlay: the game's HUD draw panicked, the HUD is off until the game is started again";

#[derive(Default)]
pub struct HudGuard {
    off_after_panic: bool,
}

impl HudGuard {
    pub fn run(&mut self, draw: impl FnOnce()) {
        if self.off_after_panic {
            return;
        }
        if catch_unwind(AssertUnwindSafe(draw)).is_err() {
            self.off_after_panic = true;
            log_line(HUD_PANICKED_LINE);
        }
    }

    pub fn is_off(&self) -> bool {
        self.off_after_panic
    }
}
