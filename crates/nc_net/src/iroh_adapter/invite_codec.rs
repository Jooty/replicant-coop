use data_encoding::BASE32_NOPAD;
use iroh::{EndpointAddr, EndpointId};

use crate::iroh_adapter::{ADDRESS_INVITE_PREFIX, INVITE_PREFIX};
use crate::transport::{Invite, NetError};

/// What a ticket names: the host's key, and for a ticket of the older form the addresses it carried, which
/// are hints the lookup adds to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ReadInvite {
    pub(super) hints: EndpointAddr,
    pub(super) older_form: bool,
}

pub(super) fn encode_invite(id: EndpointId) -> Result<Invite, NetError> {
    let key = BASE32_NOPAD.encode(id.as_bytes()).to_ascii_lowercase();
    Invite::new(format!("{INVITE_PREFIX}{key}"))
}

pub(super) fn decode_invite(invite: &Invite) -> Result<ReadInvite, NetError> {
    let text = invite.as_str();
    if let Some(body) = text.strip_prefix(INVITE_PREFIX) {
        let key: [u8; 32] = base32(body)?
            .try_into()
            .map_err(|_| NetError::InvalidInvite)?;
        let id = EndpointId::from_bytes(&key).map_err(|_| NetError::InvalidInvite)?;
        return Ok(ReadInvite {
            hints: EndpointAddr::new(id),
            older_form: false,
        });
    }
    let body = text
        .strip_prefix(ADDRESS_INVITE_PREFIX)
        .ok_or(NetError::InvalidInvite)?;
    let hints = postcard::from_bytes::<EndpointAddr>(&base32(body)?)
        .map_err(|_| NetError::InvalidInvite)?;
    Ok(ReadInvite {
        hints,
        older_form: true,
    })
}

/// Whether a ticket's text reads as a host this transport can look up, checked without connecting.
pub fn invite_reads(text: &str) -> bool {
    Invite::new(text.to_owned()).is_ok_and(|invite| decode_invite(&invite).is_ok())
}

fn base32(body: &str) -> Result<Vec<u8>, NetError> {
    BASE32_NOPAD
        .decode(body.to_ascii_uppercase().as_bytes())
        .map_err(|_| NetError::InvalidInvite)
}
