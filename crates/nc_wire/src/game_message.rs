use std::fmt::Debug;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{LinkPayload, WireError};

pub trait GameMessage: Clone + Debug + PartialEq + Serialize + DeserializeOwned {
    const PROTOCOL_VERSION: u16;
    const MAX_PLAYERS: u8;

    fn from_link(payload: LinkPayload) -> Self;

    fn into_link(self) -> Result<LinkPayload, Self>;

    fn is_relayed(&self) -> bool;

    fn is_latest_wins(&self) -> bool;

    fn ordered_tick(&self) -> Option<u32>;

    fn forwarded_to_other_guests(&self) -> bool {
        false
    }

    fn validate(&self) -> Result<(), WireError>;
}
