mod api;
mod error;
mod event;
mod invite;
mod peer;
mod stats;

pub use api::Transport;
pub use error::NetError;
pub use event::{Channel, DisconnectReason, TransportEvent};
pub use invite::Invite;
pub use peer::PeerId;
pub use stats::LinkStats;

pub const MAX_INVITE_BYTES: usize = 256;
pub const MAX_PAYLOAD_BYTES: usize = nc_wire::MAX_FRAME_BYTES;
