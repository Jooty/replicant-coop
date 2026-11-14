/// The reason a lobby ended in a failure, kept for the home row of the game it was on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LobbyFailure {
    pub game_id: &'static str,
    pub reason: String,
}
