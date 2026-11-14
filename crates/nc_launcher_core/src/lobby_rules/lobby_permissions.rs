#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LobbyPermissions {
    pub change_mode: bool,
    pub change_options: bool,
}

pub fn lobby_permissions(viewer_is_guest: bool) -> LobbyPermissions {
    let host = !viewer_is_guest;
    LobbyPermissions {
        change_mode: host,
        change_options: host,
    }
}
