use replicant_game::story_progress::{SLOT_BYTES, SeatError, check_seat};
use replicant_proto::{SEAT_PART_BYTES, SeatPart, SeatRefusal};

use crate::seat_transfer::seat_checksum;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssemblerStep {
    Progress { parts_done: u16, parts: u16 },
    Whole(Vec<u8>),
    Refused(SeatRefusal),
    Ignored,
}

struct Partial {
    transfer: u32,
    parts: u16,
    checksum: u32,
    bytes: Vec<u8>,
    next_part: u16,
}

#[derive(Default)]
pub struct SeatAssembler {
    partial: Option<Partial>,
    refused_transfer: Option<u32>,
}

impl SeatAssembler {
    pub fn accept(&mut self, part: &SeatPart) -> AssemblerStep {
        if part.part == 0 {
            self.partial = None;
            self.refused_transfer = None;
            return self.begin(part);
        }
        if self.refused_transfer == Some(part.transfer) {
            return AssemblerStep::Ignored;
        }
        let in_place = self.partial.as_ref().is_some_and(|partial| {
            partial.transfer == part.transfer
                && partial.parts == part.parts
                && partial.checksum == part.checksum
                && partial.next_part == part.part
                && partial.bytes.len() == usize::from(part.part) * SEAT_PART_BYTES
        });
        if !in_place {
            return self.refuse(part.transfer, SeatRefusal::OutOfOrder);
        }
        self.append(part)
    }

    fn begin(&mut self, part: &SeatPart) -> AssemblerStep {
        if part.total_bytes as usize != SLOT_BYTES {
            return self.refuse(part.transfer, SeatRefusal::WrongSize);
        }
        self.partial = Some(Partial {
            transfer: part.transfer,
            parts: part.parts,
            checksum: part.checksum,
            bytes: Vec::with_capacity(SLOT_BYTES),
            next_part: 0,
        });
        self.append(part)
    }

    fn append(&mut self, part: &SeatPart) -> AssemblerStep {
        let Some(partial) = self.partial.as_mut() else {
            return AssemblerStep::Ignored;
        };
        let expected = SLOT_BYTES
            .saturating_sub(usize::from(part.part) * SEAT_PART_BYTES)
            .min(SEAT_PART_BYTES);
        if part.bytes.len() != expected {
            return self.refuse(part.transfer, SeatRefusal::WrongSize);
        }
        partial.bytes.extend_from_slice(&part.bytes);
        partial.next_part += 1;
        if partial.next_part < partial.parts {
            return AssemblerStep::Progress {
                parts_done: partial.next_part,
                parts: partial.parts,
            };
        }
        let Some(finished) = self.partial.take() else {
            return AssemblerStep::Ignored;
        };
        match verdict(&finished) {
            Ok(()) => AssemblerStep::Whole(finished.bytes),
            Err(refusal) => self.refuse(part.transfer, refusal),
        }
    }

    fn refuse(&mut self, transfer: u32, refusal: SeatRefusal) -> AssemblerStep {
        self.partial = None;
        self.refused_transfer = Some(transfer);
        AssemblerStep::Refused(refusal)
    }
}

fn verdict(finished: &Partial) -> Result<(), SeatRefusal> {
    if finished.bytes.len() != SLOT_BYTES {
        return Err(SeatRefusal::WrongSize);
    }
    if seat_checksum(&finished.bytes) != finished.checksum {
        return Err(SeatRefusal::BadChecksum);
    }
    check_seat(&finished.bytes).map_err(|error| match error {
        SeatError::SlotSizeWrong => SeatRefusal::WrongSize,
        _ => SeatRefusal::NotASave,
    })
}
