use replicant_proto::GameReadiness;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerStatus {
    NotReady,
    Ready,
    StartingGame,
    InGame,
    GameClosed,
}

impl PlayerStatus {
    pub fn text(self) -> &'static str {
        match self {
            Self::NotReady => "Not ready",
            Self::Ready => "Ready",
            Self::StartingGame => "Starting game",
            Self::InGame => "In game",
            Self::GameClosed => "Game closed",
        }
    }

    /// Ready and in game are fine; every other status is something a player still waits on.
    pub fn is_fine(self) -> bool {
        matches!(self, Self::Ready | Self::InGame)
    }
}

pub fn player_status(
    lobby_ready: bool,
    game_readiness: GameReadiness,
    game_closed_after_link: bool,
) -> PlayerStatus {
    if game_closed_after_link {
        return PlayerStatus::GameClosed;
    }
    match game_readiness {
        GameReadiness::InPlay => PlayerStatus::InGame,
        GameReadiness::Loading => PlayerStatus::StartingGame,
        GameReadiness::NotLinked if lobby_ready => PlayerStatus::Ready,
        GameReadiness::NotLinked => PlayerStatus::NotReady,
    }
}
