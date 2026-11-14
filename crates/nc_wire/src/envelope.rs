use crate::{GameMessage, WireError, decode, encode};

pub const ENVELOPE_BYTES: usize = 2;
pub const ADDRESSED_TO_EVERYONE: u8 = 0xFF;

/// Who a frame after the handshake is from, and which player it is for (`None`: everyone).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Envelope {
    pub from: u8,
    pub to: Option<u8>,
}

/// Writes `[from, to]` in front of the encoded message, `to = None` as [`ADDRESSED_TO_EVERYONE`].
pub fn encode_enveloped<M: GameMessage>(
    envelope: &Envelope,
    message: &M,
) -> Result<Vec<u8>, WireError> {
    if envelope.from == ADDRESSED_TO_EVERYONE {
        return Err(WireError::OutOfRange { field: "from" });
    }
    let to = match envelope.to {
        None => ADDRESSED_TO_EVERYONE,
        Some(ADDRESSED_TO_EVERYONE) => return Err(WireError::OutOfRange { field: "to" }),
        Some(slot) => slot,
    };
    let body = encode(message)?;
    let mut frame = Vec::with_capacity(ENVELOPE_BYTES + body.len());
    frame.push(envelope.from);
    frame.push(to);
    frame.extend_from_slice(&body);
    Ok(frame)
}

/// Reads a frame written by [`encode_enveloped`]; the body keeps every limit of [`decode`].
pub fn decode_enveloped<M: GameMessage>(frame: &[u8]) -> Result<(Envelope, M), WireError> {
    if frame.is_empty() {
        return Err(WireError::Empty);
    }
    let [from, to, body @ ..] = frame else {
        return Err(WireError::Truncated);
    };
    if *from == ADDRESSED_TO_EVERYONE {
        return Err(WireError::OutOfRange { field: "from" });
    }
    let envelope = Envelope {
        from: *from,
        to: (*to != ADDRESSED_TO_EVERYONE).then_some(*to),
    };
    Ok((envelope, decode(body)?))
}
