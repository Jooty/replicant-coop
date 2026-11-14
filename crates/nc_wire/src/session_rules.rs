use serde::{Deserialize, Serialize};

pub const MAX_SESSION_SLOTS: u8 = 16;
pub const STORY_MODE: u8 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRules {
    pub mode: u8,
    pub max_players: u8,
}
