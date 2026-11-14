use crate::net::remote_player::RemotePlayer;

/// Another player in the session, by slot; `watch` is the game's own record of that player.
#[derive(Debug, Clone, PartialEq)]
pub struct Player<P, Y> {
    pub slot: u8,
    pub name: String,
    pub latest: Option<RemotePlayer<P>>,
    pub watch: Y,
}

impl<P, Y: Default> Player<P, Y> {
    pub fn joined(slot: u8, name: String) -> Self {
        Self {
            slot,
            name,
            latest: None,
            watch: Y::default(),
        }
    }
}
