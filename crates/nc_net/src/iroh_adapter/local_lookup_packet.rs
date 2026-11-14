use iroh::EndpointId;

use crate::iroh_adapter::LOCAL_LOOKUP_MAGIC;

const QUERY_KIND: u8 = 0;
const ANSWER_KIND: u8 = 1;
const KEY_BYTES: usize = 32;
const HEADER_BYTES: usize = LOCAL_LOOKUP_MAGIC.len() + 1;
const QUERY_BYTES: usize = HEADER_BYTES + KEY_BYTES;
const ANSWER_BYTES: usize = QUERY_BYTES + 2;

/// A packet of the local network lookup: a question for the endpoint that holds a key, or that endpoint's
/// answer with the UDP port it takes connections on. The answer's address is the packet's own source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LocalLookupPacket {
    Query {
        key: [u8; KEY_BYTES],
    },
    Answer {
        key: [u8; KEY_BYTES],
        ipv4_port: u16,
    },
}

impl LocalLookupPacket {
    pub(super) fn query(id: EndpointId) -> Self {
        Self::Query {
            key: *id.as_bytes(),
        }
    }

    pub(super) fn to_bytes(self) -> Vec<u8> {
        let mut bytes = LOCAL_LOOKUP_MAGIC.to_vec();
        match self {
            Self::Query { key } => {
                bytes.push(QUERY_KIND);
                bytes.extend_from_slice(&key);
            }
            Self::Answer { key, ipv4_port } => {
                bytes.push(ANSWER_KIND);
                bytes.extend_from_slice(&key);
                bytes.extend_from_slice(&ipv4_port.to_be_bytes());
            }
        }
        bytes
    }

    pub(super) fn read(bytes: &[u8]) -> Option<Self> {
        let body = bytes.strip_prefix(LOCAL_LOOKUP_MAGIC)?;
        let (&kind, rest) = body.split_first()?;
        let key: [u8; KEY_BYTES] = rest.get(..KEY_BYTES)?.try_into().ok()?;
        match (kind, bytes.len()) {
            (QUERY_KIND, QUERY_BYTES) => Some(Self::Query { key }),
            (ANSWER_KIND, ANSWER_BYTES) => {
                let port = rest.get(KEY_BYTES..)?.try_into().ok()?;
                Some(Self::Answer {
                    key,
                    ipv4_port: u16::from_be_bytes(port),
                })
            }
            _ => None,
        }
    }
}
