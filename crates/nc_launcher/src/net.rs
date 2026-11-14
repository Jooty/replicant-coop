mod stub;

pub use nc_launcher_core::{
    ConnectionReport, ConnectionState, LOBBY_MODE_CONTINUE, LOBBY_MODE_NEW_GAME, LauncherCore,
    LauncherEvent, LauncherNet, LinkStatus, LobbyReady, MAX_LOBBY_SLOT, MemberChange, MemberStatus,
    NetError, NetState, NetStatus, PathKind, PlayerSettings, RttStats, SessionOptions,
};
pub use stub::StubNet;

pub const DEFAULT_PLAYER_NAME: &str = nc_launcher_core::DEFAULT_PLAYER_NAME;
