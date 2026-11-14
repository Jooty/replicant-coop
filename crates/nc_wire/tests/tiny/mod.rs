use nc_wire::{GameMessage, LinkPayload, Reject, WireError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Tiny {
    Link(Reject),
    Blob(Vec<u8>),
    Forbidden,
}

impl GameMessage for Tiny {
    const PROTOCOL_VERSION: u16 = 1;
    const MAX_PLAYERS: u8 = 2;

    fn from_link(payload: LinkPayload) -> Self {
        match payload {
            LinkPayload::Reject(reject) => Tiny::Link(reject),
            _ => Tiny::Forbidden,
        }
    }

    fn into_link(self) -> Result<LinkPayload, Self> {
        match self {
            Tiny::Link(reject) => Ok(LinkPayload::Reject(reject)),
            other => Err(other),
        }
    }

    fn is_relayed(&self) -> bool {
        matches!(self, Tiny::Blob(_))
    }

    fn is_latest_wins(&self) -> bool {
        false
    }

    fn ordered_tick(&self) -> Option<u32> {
        None
    }

    fn validate(&self) -> Result<(), WireError> {
        match self {
            Tiny::Forbidden => Err(WireError::OutOfRange { field: "kind" }),
            _ => Ok(()),
        }
    }
}
