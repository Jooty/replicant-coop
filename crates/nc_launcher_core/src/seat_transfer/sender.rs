use replicant_game::story_progress::{SeatError, check_seat};
use replicant_proto::{SEAT_PART_BYTES, SeatPart, SeatRefusal};

use crate::seat_transfer::seat_checksum;

pub struct SeatSender {
    transfer: u32,
    seat: Vec<u8>,
    checksum: u32,
    parts: u16,
    next_part: u16,
}

impl SeatSender {
    pub fn new(transfer: u32, seat: Vec<u8>) -> Result<Self, SeatRefusal> {
        check_seat(&seat).map_err(|error| match error {
            SeatError::SlotSizeWrong => SeatRefusal::WrongSize,
            _ => SeatRefusal::NotASave,
        })?;
        let parts = u16::try_from(seat.len().div_ceil(SEAT_PART_BYTES))
            .map_err(|_| SeatRefusal::WrongSize)?;
        Ok(Self {
            transfer,
            checksum: seat_checksum(&seat),
            seat,
            parts,
            next_part: 0,
        })
    }

    pub fn transfer(&self) -> u32 {
        self.transfer
    }

    pub fn parts(&self) -> u16 {
        self.parts
    }

    pub fn parts_sent(&self) -> u16 {
        self.next_part
    }

    pub fn is_all_sent(&self) -> bool {
        self.next_part >= self.parts
    }

    pub fn next_parts(&mut self, at_most: usize) -> Vec<SeatPart> {
        let mut batch = Vec::new();
        while batch.len() < at_most && !self.is_all_sent() {
            let start = usize::from(self.next_part) * SEAT_PART_BYTES;
            let end = (start + SEAT_PART_BYTES).min(self.seat.len());
            batch.push(SeatPart {
                transfer: self.transfer,
                part: self.next_part,
                parts: self.parts,
                total_bytes: u32::try_from(self.seat.len()).unwrap_or(u32::MAX),
                checksum: self.checksum,
                bytes: self.seat[start..end].to_vec(),
            });
            self.next_part += 1;
        }
        batch
    }
}
