use nc_wire::{STORY_MODE, SessionRules};

pub const CAMPAIGN_PLAYER_COUNT: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameMode {
    pub id: u8,
    pub label: &'static str,
    pub max_players: u8,
}

pub const CO_OP_CAMPAIGN: GameMode = GameMode {
    id: STORY_MODE,
    label: "Co-op campaign",
    max_players: CAMPAIGN_PLAYER_COUNT,
};

impl GameMode {
    pub fn is_campaign(&self) -> bool {
        self.id == STORY_MODE
    }

    pub fn rules(&self) -> SessionRules {
        SessionRules {
            mode: self.id,
            max_players: self.max_players,
        }
    }
}
