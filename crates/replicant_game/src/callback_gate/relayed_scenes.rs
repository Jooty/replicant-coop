use crate::callback_gate::{GATED_EVENT_SUFFIX, SCENE_PLAY_SUFFIXES, STORY_EVENT_PREFIXES};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayedStart {
    Run,
    Drop { first_for_start: bool },
}

/// The events whose scene played here from the story relay in this phase. A start of one of
/// them that the game fires later would play the scene again, and spawn what it spawns again.
#[derive(Debug, Clone, Default)]
pub struct RelayedScenesPlayed {
    phase: Option<u16>,
    bases: Vec<String>,
    drops_logged: Vec<String>,
}

impl RelayedScenesPlayed {
    pub const fn new() -> Self {
        Self {
            phase: None,
            bases: Vec::new(),
            drops_logged: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Notes `function` when it is an event's scene; true when it was one.
    pub fn note_played(&mut self, phase: u16, function: &str) -> bool {
        let Some(base) = scene_base(function) else {
            return false;
        };
        if self.phase != Some(phase) {
            self.reset();
            self.phase = Some(phase);
        }
        if !self.bases.iter().any(|known| known == base) {
            self.bases.push(base.to_owned());
        }
        true
    }

    pub fn played(&self, phase: u16, start: &str) -> bool {
        self.phase == Some(phase)
            && start_base(start).is_some_and(|base| self.bases.iter().any(|known| known == base))
    }

    pub fn judge_start(&mut self, phase: u16, start: &str) -> PlayedStart {
        if !self.played(phase, start) {
            return PlayedStart::Run;
        }
        let first_for_start = !self.drops_logged.iter().any(|known| known == start);
        if first_for_start {
            self.drops_logged.push(start.to_owned());
        }
        PlayedStart::Drop { first_for_start }
    }
}

pub fn scene_base(function: &str) -> Option<&str> {
    if !STORY_EVENT_PREFIXES
        .iter()
        .any(|prefix| function.starts_with(prefix))
    {
        return None;
    }
    let trimmed = function.trim_end_matches(|character: char| character.is_ascii_digit());
    // The longest suffix comes first: "_PLAY_EVENT" also ends in "_EVENT".
    SCENE_PLAY_SUFFIXES
        .iter()
        .find_map(|suffix| trimmed.strip_suffix(suffix))
        .filter(|base| !base.is_empty())
}

pub fn start_base(name: &str) -> Option<&str> {
    name.trim_end_matches(|character: char| character.is_ascii_digit())
        .strip_suffix(GATED_EVENT_SUFFIX)
        .filter(|base| !base.is_empty())
}
