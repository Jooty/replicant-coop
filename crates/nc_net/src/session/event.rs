use nc_wire::{GameMessage, MemberPing, RejectReason, SessionRules};

use crate::transport::PeerId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaveReason {
    Quit,
    TimedOut,
    ProtocolViolation,
    TransportClosed,
    RemovedByHost,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SessionEvent<M: GameMessage> {
    /// `peer` is the transport link the player is reached over: on a guest, always the host's.
    PlayerJoined {
        slot: u8,
        name: String,
        peer: PeerId,
    },
    PlayerLeft {
        slot: u8,
        reason: LeaveReason,
        peer: PeerId,
    },
    /// An ordered or relayed message; `from` is the sender's slot as the host stamped it.
    Message {
        from: u8,
        message: M,
    },
    Rejected {
        reason: RejectReason,
    },
    RulesChanged(SessionRules),
    MemberPings(Vec<MemberPing>),
    PeerRefused {
        peer: PeerId,
        reason: RejectReason,
    },
}

impl LeaveReason {
    pub fn label(self) -> &'static str {
        match self {
            LeaveReason::Quit => "the other player closed the link",
            LeaveReason::TimedOut => "no traffic from the other player (timed out)",
            LeaveReason::ProtocolViolation => "the other player sent something invalid",
            LeaveReason::TransportClosed => "the network link closed or failed",
            LeaveReason::RemovedByHost => "removed by the host",
        }
    }
}
