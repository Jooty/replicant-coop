/// The kind of game (new or saved, a lobby mode) this launcher's lobby runs. A guest's follows the kind the
/// host's lobby state names once it has arrived, whichever tab the guest joined from; the host's, and a
/// guest's before the host's state arrives, is the tab's own.
pub fn followed_lobby_kind(tab_kind: u8, viewer_is_guest: bool, host_kind: Option<u8>) -> u8 {
    match host_kind {
        Some(kind) if viewer_is_guest => kind,
        _ => tab_kind,
    }
}
