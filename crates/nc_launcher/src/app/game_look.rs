use nc_launcher_core::{GameProfile, REPLICANT};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMark {
    Circle,
    Diamond,
}

/// How a game's name is set: NieR Replicant in EB Garamond, any other name in light spaced capitals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleFace {
    Garamond,
    SpacedCapitals,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameLook {
    pub mark: GameMark,
    pub title_face: TitleFace,
}

pub fn game_look(profile: &GameProfile) -> GameLook {
    if profile.game_id == REPLICANT.game_id {
        GameLook {
            mark: GameMark::Circle,
            title_face: TitleFace::Garamond,
        }
    } else {
        neutral_look()
    }
}

pub fn neutral_look() -> GameLook {
    GameLook {
        mark: GameMark::Circle,
        title_face: TitleFace::SpacedCapitals,
    }
}
