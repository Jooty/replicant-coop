mod config;
mod driver;
mod event;
mod peer;

use std::time::Duration;

pub use config::{Role, SessionConfig};
pub use driver::{PingCounts, Session};
pub use event::{LeaveReason, SessionEvent};

pub const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(1);
pub const PEER_TIMEOUT: Duration = Duration::from_secs(5);
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
pub const MEMBER_PING_LIST_INTERVAL: Duration = Duration::from_secs(3);
pub const REFUSAL_LINGER: Duration = Duration::from_millis(500);
