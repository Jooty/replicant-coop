use serde::{Deserialize, Serialize};

use crate::wire::WireError;

pub const SEAT_PART_BYTES: usize = 480;
pub const MAX_SEAT_TRANSFER_BYTES: u32 = 40_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatPart {
    pub transfer: u32,
    pub part: u16,
    pub parts: u16,
    pub total_bytes: u32,
    pub checksum: u32,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeatRefusal {
    WrongSize,
    OutOfOrder,
    BadChecksum,
    NotASave,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeatOutcome {
    ReceivedWholeAndValid,
    Refused(SeatRefusal),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatReceipt {
    pub transfer: u32,
    pub outcome: SeatOutcome,
}

impl SeatPart {
    pub fn validate(&self) -> Result<(), WireError> {
        if self.total_bytes == 0 || self.total_bytes > MAX_SEAT_TRANSFER_BYTES {
            return Err(WireError::OutOfRange {
                field: "total_bytes",
            });
        }
        if usize::from(self.parts) != parts_for(self.total_bytes) {
            return Err(WireError::OutOfRange { field: "parts" });
        }
        if self.part >= self.parts {
            return Err(WireError::OutOfRange { field: "part" });
        }
        if self.bytes.is_empty() || self.bytes.len() > SEAT_PART_BYTES {
            return Err(WireError::OutOfRange { field: "bytes" });
        }
        Ok(())
    }
}

pub fn parts_for(total_bytes: u32) -> usize {
    (total_bytes as usize).div_ceil(SEAT_PART_BYTES)
}
