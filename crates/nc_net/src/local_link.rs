mod frame;
mod transport;

pub use transport::LocalLinkTransport;

pub const LINK_VERSION: u16 = 3;
pub const LINK_ADDRESS: &str = "127.0.0.1";
pub const LINK_INVITE_PREFIX: &str = "link-";
pub const LINK_LAUNCHER_PEER: crate::transport::PeerId = crate::transport::PeerId([0x4c; 32]);
pub const LINK_GAME_PEER: crate::transport::PeerId = crate::transport::PeerId([0x47; 32]);
