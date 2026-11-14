use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::menu::{AreaChangeTracker, SectionKind};

/// What the menu remembers between frames: typed text, the last messages, which sections are open and which
/// panels are shown.
#[derive(Debug)]
pub struct MenuState {
    pub(crate) started: Instant,
    pub(crate) area_filter: String,
    pub(crate) area_changes: AreaChangeTracker,
    pub(crate) teleport_message: String,
    pub(crate) story_jump_message: String,
    pub(crate) fix_message: String,
    pub(crate) open_sections: HashSet<SectionKind>,
    pub(crate) open_panels: HashMap<u32, bool>,
}

impl MenuState {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            area_filter: String::new(),
            area_changes: AreaChangeTracker::default(),
            teleport_message: String::new(),
            story_jump_message: String::new(),
            fix_message: String::new(),
            open_sections: HashSet::from([SectionKind::CoOp]),
            open_panels: HashMap::new(),
        }
    }

    pub fn set_section_open(&mut self, section: SectionKind, open: bool) {
        if open {
            self.open_sections.insert(section);
        } else {
            self.open_sections.remove(&section);
        }
    }

    pub fn section_is_open(&self, section: SectionKind) -> bool {
        self.open_sections.contains(&section)
    }

    pub fn set_panel_open(&mut self, panel_id: u32, open: bool) {
        self.open_panels.insert(panel_id, open);
    }

    pub fn panel_is_open(&self, panel_id: u32) -> bool {
        self.open_panels.get(&panel_id).copied().unwrap_or(false)
    }

    pub(crate) fn now_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

impl Default for MenuState {
    fn default() -> Self {
        Self::new()
    }
}
