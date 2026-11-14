/// Whether a game's co-op saves may be deleted now, and if not, why not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoopSaveDeleteOffer {
    Allowed,
    NothingToDelete,
    GameRunning,
    LobbyOpen,
}

impl CoopSaveDeleteOffer {
    pub fn allowed(self) -> bool {
        self == Self::Allowed
    }

    /// The launcher log's words for a refused delete.
    pub fn refusal_line(self) -> Option<&'static str> {
        match self {
            Self::Allowed => None,
            Self::NothingToDelete => Some("there are no co-op saves to delete"),
            Self::GameRunning => Some("close the game before deleting its co-op saves"),
            Self::LobbyOpen => Some("leave the lobby before deleting the game's co-op saves"),
        }
    }
}

/// Deleting needs something to delete, and never happens while that game runs or a lobby of it is open.
pub fn coop_save_delete_offer(
    coop_saves: usize,
    game_running: bool,
    lobby_open: bool,
) -> CoopSaveDeleteOffer {
    if coop_saves == 0 {
        CoopSaveDeleteOffer::NothingToDelete
    } else if game_running {
        CoopSaveDeleteOffer::GameRunning
    } else if lobby_open {
        CoopSaveDeleteOffer::LobbyOpen
    } else {
        CoopSaveDeleteOffer::Allowed
    }
}
