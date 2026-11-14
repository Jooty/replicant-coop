pub const FIXED_CAMERA_SET_EYE_RVA: u32 = 0x34_0F50;
pub const EYE_SET_CALLER_SLOTS: usize = 32;
pub const EYE_SET_CALLERS_LISTED: usize = 6;

/// Counts the fixed camera's `SetEye` 0x340F50 calls in a fight window: by the mod's own camera
/// calls, and by the game per caller (the return address as an offset from the image base).
/// Fixed size, so recording never allocates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EyeSetTally {
    callers: [(u32, u32); EYE_SET_CALLER_SLOTS],
    used: usize,
    by_game: u32,
    by_mod: u32,
    callers_not_kept: u32,
}

/// What one stretch of the tally held, callers most frequent first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EyeSetStretch {
    pub by_game: u32,
    pub by_mod: u32,
    pub callers: Vec<(u32, u32)>,
    pub callers_not_kept: u32,
}

impl Default for EyeSetTally {
    fn default() -> Self {
        Self::new()
    }
}

impl EyeSetTally {
    pub const fn new() -> Self {
        Self {
            callers: [(0, 0); EYE_SET_CALLER_SLOTS],
            used: 0,
            by_game: 0,
            by_mod: 0,
            callers_not_kept: 0,
        }
    }

    pub fn record(&mut self, caller_offset: u32, by_the_mod: bool) {
        if by_the_mod {
            self.by_mod = self.by_mod.saturating_add(1);
            return;
        }
        self.by_game = self.by_game.saturating_add(1);
        let used = &mut self.callers[..self.used];
        if let Some(slot) = used.iter_mut().find(|(offset, _)| *offset == caller_offset) {
            slot.1 = slot.1.saturating_add(1);
        } else if self.used < EYE_SET_CALLER_SLOTS {
            self.callers[self.used] = (caller_offset, 1);
            self.used += 1;
        } else {
            self.callers_not_kept = self.callers_not_kept.saturating_add(1);
        }
    }

    /// Everything counted since the last take, and a fresh start.
    pub fn take(&mut self) -> EyeSetStretch {
        let mut callers = self.callers[..self.used].to_vec();
        callers.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        callers.truncate(EYE_SET_CALLERS_LISTED);
        let stretch = EyeSetStretch {
            by_game: self.by_game,
            by_mod: self.by_mod,
            callers,
            callers_not_kept: self.callers_not_kept,
        };
        *self = Self::new();
        stretch
    }
}

pub fn eye_set_text(stretch: &EyeSetStretch) -> String {
    let callers: Vec<String> = stretch
        .callers
        .iter()
        .map(|(offset, count)| format!("{offset:#X} x{count}"))
        .collect();
    format!(
        "eye set by the game {} by the mod {}; game callers [{}]{}",
        stretch.by_game,
        stretch.by_mod,
        callers.join(", "),
        if stretch.callers_not_kept > 0 {
            format!(
                " (+{} from callers past the table)",
                stretch.callers_not_kept
            )
        } else {
            String::new()
        }
    )
}
