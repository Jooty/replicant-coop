#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryJumpEntry {
    pub id: u32,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixEntry {
    pub id: u32,
    pub label: String,
    pub disabled_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixOutcome {
    Done,
    CopyToClipboard(String),
    Refused(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtraPanel {
    pub id: u32,
    pub label: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteTone {
    Plain,
    Warn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuNote {
    pub text: String,
    pub tone: NoteTone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerLine {
    pub name: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CoOpStatus {
    pub link: String,
    pub connected: bool,
    pub players: Vec<PlayerLine>,
    pub ping_ms: Option<u32>,
}

impl MenuNote {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            tone: NoteTone::Plain,
        }
    }

    pub fn warn(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            tone: NoteTone::Warn,
        }
    }
}
