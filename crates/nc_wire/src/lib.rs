mod build_gate;
mod codec;
mod envelope;
mod error;
mod game_message;
mod link;
mod session_rules;

pub use build_gate::BuildGate;
pub use codec::{decode, encode};
pub use envelope::{
    ADDRESSED_TO_EVERYONE, ENVELOPE_BYTES, Envelope, decode_enveloped, encode_enveloped,
};
pub use error::WireError;
pub use game_message::GameMessage;
pub use link::{Hello, LinkPayload, Member, MemberPing, Ping, Pong, Reject, RejectReason, Welcome};
pub use session_rules::{MAX_SESSION_SLOTS, STORY_MODE, SessionRules};

pub const MAX_MESSAGE_BYTES: usize = 512;
pub const MAX_FRAME_BYTES: usize = MAX_MESSAGE_BYTES + ENVELOPE_BYTES;
