mod first_tick;
mod link_config;
mod link_peer;
mod net_clock;
mod net_game;
mod net_shared;
mod net_state;
mod net_thread;
mod player;
mod readiness_log;
mod remote_player;
mod story_partner;
mod worker;

use std::time::Duration;

pub use first_tick::first_state_tick;
pub use link_config::{
    AUTO_CONFIG_FILE_NAME, ConfigError, DEFAULT_LOG_FILE_NAME, LINK_ENV, LOG_ENV, NAME_ENV,
    game_folder, parse_env_flag, parse_env_number, read_config_text, resolve_log_path,
};
pub use link_peer::LinkPeer;
pub use net_clock::net_clock_ms;
pub use net_game::{LinkAccess, NetGame, NetSharedOf, PlayerOf, SharedNetOf};
pub use net_shared::{NetShared, SharedNet};
pub use net_state::NetState;
pub use net_thread::NetThread;
pub use player::Player;
pub use readiness_log::log_readiness;
pub use remote_player::RemotePlayer;

pub const SEND_INTERVAL: Duration = Duration::from_millis(33);
pub const WORKER_WAKE_INTERVAL: Duration = Duration::from_millis(4);
pub const STATE_TICK_CLOCK_STEP_MS: u128 = 16;
pub const MAX_RELAYED_INBOX: usize = 512;
