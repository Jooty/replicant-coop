use serde::{Deserialize, Serialize};

use crate::wire::WireError;

pub const NO_MOTION_NAME_HASH: u32 = 0;
pub const MAX_BODY_PACK_NAME_BYTES: usize = 16;
const FNV_OFFSET_BASIS: u32 = 0x811C_9DC5;
const FNV_PRIME: u32 = 0x0100_0193;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartnerBody {
    #[default]
    #[serde(rename = "nier")]
    Nier,
    #[serde(rename = "prologue")]
    PrologueCharacter,
    #[serde(rename = "nierY")]
    SeedNierY,
    #[serde(rename = "nierF")]
    SeedNierF,
    #[serde(rename = "kaine", alias = "kaineE")]
    SeedKaineE,
}

pub const HOST_BODY: PartnerBody = PartnerBody::Nier;
pub const GUEST_BODY: PartnerBody = PartnerBody::SeedKaineE;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyPackName(String);

impl BodyPackName {
    pub fn new(name: &str) -> Result<Self, WireError> {
        let valid = !name.is_empty()
            && name.len() <= MAX_BODY_PACK_NAME_BYTES
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
        if valid {
            Ok(Self(name.to_owned()))
        } else {
            Err(WireError::OutOfRange {
                field: "body pack name",
            })
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn validate(&self) -> Result<(), WireError> {
        Self::new(&self.0).map(|_| ())
    }
}

pub fn motion_name_hash(name: &str) -> u32 {
    let hash = name.bytes().fold(FNV_OFFSET_BASIS, |hash, byte| {
        hash.wrapping_mul(FNV_PRIME) ^ u32::from(byte)
    });
    if hash == NO_MOTION_NAME_HASH { 1 } else { hash }
}
