use crate::net::LinkStatus;

/// A lobby that ended without the player asking: the one line saying what happened, and the link as it last
/// was, for the seats the page keeps showing.
#[derive(Debug, Clone, PartialEq)]
pub struct EndedLobby {
    pub reason: String,
    pub last_seen: LinkStatus,
}
