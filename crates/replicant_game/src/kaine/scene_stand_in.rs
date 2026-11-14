use crate::kaine::scene_stand_ins::{SceneStandIn, scene_stand_in_opened_by};
use crate::kaine::{RowOrigin, SCENE_STAND_IN_MAX_OPEN_MS, StandInSide};

/// What the guest's game knows this tick. `closing_flag_on` is the host's flag that closes the
/// open scene (the guest's copy of the host's story); `pack` is this game's area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneStandInFacts<'a> {
    pub side: StandInSide,
    pub in_session: bool,
    pub pack: Option<&'a str>,
    pub closing_flag_on: bool,
    pub free_to_move: bool,
    pub now_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SceneStandInEvent {
    Opened {
        row: &'static SceneStandIn,
    },
    /// Put the guest at the companion's place now.
    Place {
        row: &'static SceneStandIn,
    },
    Released {
        row: &'static SceneStandIn,
        placed: bool,
        why: &'static str,
    },
}

/// The guest's part in a listed story scene: placed once at the companion's place when the
/// host's talk opens, free again when it ends. Nothing here waits on a menu or a window focus.
#[derive(Debug, Clone, Default)]
pub struct GuestSceneStandIn {
    pack: Option<String>,
    from_scripts: bool,
    open: Option<&'static SceneStandIn>,
    opened_at_ms: u64,
    placed: bool,
    events: Vec<SceneStandInEvent>,
}

impl GuestSceneStandIn {
    pub const fn new() -> Self {
        Self {
            pack: None,
            from_scripts: false,
            open: None,
            opened_at_ms: 0,
            placed: false,
            events: Vec::new(),
        }
    }

    pub const fn open_scene(&self) -> Option<&'static SceneStandIn> {
        self.open
    }

    /// Whether the rows generated from the scripts take part (off in a new rule). Switching them
    /// off releases a scene one of them opened.
    pub fn use_rows_from_scripts(&mut self, enabled: bool) {
        self.from_scripts = enabled;
        if !enabled
            && self
                .open
                .is_some_and(|row| row.origin == RowOrigin::FromScripts)
        {
            self.release("the rows from the scripts were switched off");
        }
    }

    /// The first line of a listed scene's talk opens it; later lines and other talks change nothing.
    /// A row from the scripts needs this game in its pack (as the last step saw it).
    pub fn partner_talk_line(&mut self, talk_id: u32, now_ms: u64) {
        if self.open.is_some() {
            return;
        }
        if let Some(row) =
            scene_stand_in_opened_by(talk_id, self.pack.as_deref(), self.from_scripts)
        {
            self.open = Some(row);
            self.opened_at_ms = now_ms;
            self.placed = false;
            self.events.push(SceneStandInEvent::Opened { row });
        }
    }

    pub fn partner_talk_ended(&mut self) {
        self.release("the host's talk ended");
    }

    /// A script function ran on this game: the scene's own end closes it.
    pub fn function_ran(&mut self, name: &str) {
        if self.open.is_some_and(|row| row.ended_by.contains(&name)) {
            self.release("the scene's end ran here");
        }
    }

    pub fn step(&mut self, facts: &SceneStandInFacts) -> Vec<SceneStandInEvent> {
        if self.pack.as_deref() != facts.pack {
            self.pack = facts.pack.map(str::to_owned);
        }
        if let Some(row) = self.open {
            if facts.side != StandInSide::Guest {
                self.release("this game is the host's, only the guest is placed");
            } else if !facts.in_session {
                self.release("the session ended");
            } else if row.origin == RowOrigin::FromScripts && facts.pack != Some(row.pack) {
                self.release("this game left the row's pack");
            } else if facts.closing_flag_on {
                self.release("the host's closing flag is on");
            } else if facts.now_ms.saturating_sub(self.opened_at_ms) > SCENE_STAND_IN_MAX_OPEN_MS {
                self.release("open too long without an end, let go");
            } else if !self.placed && facts.free_to_move && facts.pack == Some(row.pack) {
                self.placed = true;
                self.events.push(SceneStandInEvent::Place { row });
            }
        }
        std::mem::take(&mut self.events)
    }

    fn release(&mut self, why: &'static str) {
        if let Some(row) = self.open.take() {
            self.events.push(SceneStandInEvent::Released {
                row,
                placed: self.placed,
                why,
            });
            self.placed = false;
        }
    }
}
