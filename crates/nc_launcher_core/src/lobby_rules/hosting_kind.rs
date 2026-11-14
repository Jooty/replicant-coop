use replicant_proto::{LOBBY_MODE_CONTINUE, LOBBY_MODE_NEW_GAME};

/// The kind of game HOST opens the lobby on: the saved campaign when the game has a co-op save, a new one
/// when it has none. The host may still switch inside the lobby.
pub fn hosting_kind(has_coop_save: bool) -> u8 {
    if has_coop_save {
        LOBBY_MODE_CONTINUE
    } else {
        LOBBY_MODE_NEW_GAME
    }
}
