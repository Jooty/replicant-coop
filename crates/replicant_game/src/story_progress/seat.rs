use crate::story_progress::{
    PHASE_NAME_BLOCK_OFFSET, SAVE_FILE_BYTES, SAVE_FILE_SLOT_COUNT, SAVE_FILE_SLOT_MASK_OFFSET,
    SLOT_BYTES, SLOT_CHECKSUM_OFFSET, SLOT_VERSION_OFFSET, SLOT_VERSIONS_IN_USE, SLOTS_FILE_OFFSET,
    fix_header_checksum, slot_checksum,
};

const SEAT_SLOT_INDEX: usize = 0;
const SLOT_MASK_BITS_NOT_FOR_SLOTS: u8 = 0x80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeatError {
    FileSizeWrong,
    SlotSizeWrong,
    NoSuchSlot,
    NotAGameSlot,
    EmptySlot,
    ChecksumWrong,
}

pub fn check_seat(seat: &[u8]) -> Result<(), SeatError> {
    if seat.len() != SLOT_BYTES {
        return Err(SeatError::SlotSizeWrong);
    }
    let version = u32::from_le_bytes(
        seat[SLOT_VERSION_OFFSET..SLOT_VERSION_OFFSET + 4]
            .try_into()
            .map_err(|_| SeatError::SlotSizeWrong)?,
    );
    if !SLOT_VERSIONS_IN_USE.contains(&version) {
        return Err(SeatError::NotAGameSlot);
    }
    if seat[PHASE_NAME_BLOCK_OFFSET] == 0 {
        return Err(SeatError::EmptySlot);
    }
    let stored = u32::from_le_bytes(
        seat[SLOT_CHECKSUM_OFFSET..SLOT_CHECKSUM_OFFSET + 4]
            .try_into()
            .map_err(|_| SeatError::SlotSizeWrong)?,
    );
    match slot_checksum(seat) {
        Some(sum) if sum == stored => Ok(()),
        _ => Err(SeatError::ChecksumWrong),
    }
}

pub fn seat_bytes_of(file: &[u8], slot: u8) -> Result<Vec<u8>, SeatError> {
    if file.len() != SAVE_FILE_BYTES {
        return Err(SeatError::FileSizeWrong);
    }
    if usize::from(slot) >= SAVE_FILE_SLOT_COUNT {
        return Err(SeatError::NoSuchSlot);
    }
    let start = SLOTS_FILE_OFFSET + usize::from(slot) * SLOT_BYTES;
    let seat = file[start..start + SLOT_BYTES].to_vec();
    check_seat(&seat)?;
    Ok(seat)
}

pub fn session_file_with_seat(blank: &[u8], seat: &[u8]) -> Result<Vec<u8>, SeatError> {
    if blank.len() != SAVE_FILE_BYTES {
        return Err(SeatError::FileSizeWrong);
    }
    check_seat(seat)?;
    let mut file = blank.to_vec();
    let start = SLOTS_FILE_OFFSET + SEAT_SLOT_INDEX * SLOT_BYTES;
    file[start..start + SLOT_BYTES].copy_from_slice(seat);
    let kept = file[SAVE_FILE_SLOT_MASK_OFFSET] & SLOT_MASK_BITS_NOT_FOR_SLOTS;
    file[SAVE_FILE_SLOT_MASK_OFFSET] = kept | (1 << SEAT_SLOT_INDEX);
    fix_header_checksum(&mut file).map_err(|_| SeatError::FileSizeWrong)?;
    Ok(file)
}
