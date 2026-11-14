#[cfg(feature = "iroh")]
mod iroh_adapter;
pub mod link_traffic;
mod local_link;
mod loopback;
mod progress;
mod session;
mod transport;

#[cfg(feature = "iroh")]
pub use iroh_adapter::{IrohConfig, IrohTransport, LOCAL_LOOKUP_PORT, invite_reads};
pub use local_link::{
    LINK_ADDRESS, LINK_GAME_PEER, LINK_INVITE_PREFIX, LINK_LAUNCHER_PEER, LINK_VERSION,
    LocalLinkTransport,
};
pub use loopback::{LoopbackNetwork, LoopbackTransport};
pub use progress::{
    AddressKind, AddressReach, LinkProgress, LookupMeans, PathRoute, ProgressBoard, ProgressEntry,
    ProgressSink,
};
pub use session::{
    HANDSHAKE_TIMEOUT, KEEPALIVE_INTERVAL, LeaveReason, MEMBER_PING_LIST_INTERVAL, PEER_TIMEOUT,
    PingCounts, REFUSAL_LINGER, Role, Session, SessionConfig, SessionEvent,
};
pub use transport::{
    Channel, DisconnectReason, Invite, LinkStats, MAX_INVITE_BYTES, MAX_PAYLOAD_BYTES, NetError,
    PeerId, Transport, TransportEvent,
};
