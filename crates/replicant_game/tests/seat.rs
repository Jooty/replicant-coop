use replicant_game::story_progress::{
    SeatError, changed_slot, fix_header_checksum, fix_slot_checksum, header_checksum,
    seat_bytes_of, session_file_with_seat,
};

const FILE_BYTES: usize = 0x4_8200;
const SLOTS_START: usize = 0x8160;
const SLOT_BYTES: usize = 0x9260;
const MASK_OFFSET: usize = 0x6C;

fn slot_with(seed: u8) -> Vec<u8> {
    let mut slot = vec![0_u8; SLOT_BYTES];
    slot[..4].copy_from_slice(&200_u32.to_le_bytes());
    slot[4] = b'P';
    for (index, byte) in slot[0x100..0x900].iter_mut().enumerate() {
        *byte = (index as u8).wrapping_add(seed);
    }
    assert!(fix_slot_checksum(&mut slot));
    slot
}

fn file_with(slots: &[(usize, Vec<u8>)]) -> Vec<u8> {
    let mut file = vec![0_u8; FILE_BYTES];
    for (index, byte) in file[..0x8154].iter_mut().enumerate() {
        *byte = (index % 13) as u8;
    }
    file[MASK_OFFSET] = 0;
    for (slot, bytes) in slots {
        let start = SLOTS_START + slot * SLOT_BYTES;
        file[start..start + SLOT_BYTES].copy_from_slice(bytes);
        file[MASK_OFFSET] |= 1 << slot;
    }
    fix_header_checksum(&mut file).unwrap();
    file
}

#[test]
fn a_seat_lifted_and_put_back_is_the_same_slot() {
    let seat = slot_with(9);
    let file = file_with(&[(0, slot_with(1)), (3, seat.clone())]);
    let lifted = seat_bytes_of(&file, 3).unwrap();
    assert_eq!(lifted, seat);
    let session = session_file_with_seat(&file_with(&[]), &lifted).unwrap();
    assert_eq!(seat_bytes_of(&session, 0).unwrap(), seat);
}

#[test]
fn a_session_file_with_a_seat_is_valid() {
    let seat = slot_with(4);
    let mut blank = file_with(&[]);
    blank[MASK_OFFSET] = 0b0110_0000 | 0x80;
    let session = session_file_with_seat(&blank, &seat).unwrap();
    assert_eq!(session.len(), FILE_BYTES);
    assert_eq!(session[MASK_OFFSET], 0x81);
    let stored = u32::from_le_bytes(session[0x8154..0x8158].try_into().unwrap());
    assert_eq!(header_checksum(&session), Some(stored));
    assert!(replicant_game::story_progress::save_file_is_valid(&session));
    for other in 1..7 {
        let start = SLOTS_START + other * SLOT_BYTES;
        assert_eq!(
            session[start..start + SLOT_BYTES],
            blank[start..start + SLOT_BYTES]
        );
    }
}

#[test]
fn a_damaged_seat_is_refused() {
    let mut damaged = slot_with(2);
    damaged[0x200] ^= 0xFF;
    let file = file_with(&[(1, damaged.clone())]);
    assert_eq!(seat_bytes_of(&file, 1), Err(SeatError::ChecksumWrong));
    assert_eq!(
        session_file_with_seat(&file_with(&[]), &damaged),
        Err(SeatError::ChecksumWrong)
    );
    assert_eq!(seat_bytes_of(&file, 2), Err(SeatError::NotAGameSlot));
    assert_eq!(seat_bytes_of(&file, 7), Err(SeatError::NoSuchSlot));
    assert_eq!(seat_bytes_of(&file[..10], 1), Err(SeatError::FileSizeWrong));
    assert_eq!(
        session_file_with_seat(&file_with(&[]), &damaged[..10]),
        Err(SeatError::SlotSizeWrong)
    );
    let mut no_phase = slot_with(2);
    no_phase[4] = 0;
    fix_slot_checksum(&mut no_phase);
    assert_eq!(
        session_file_with_seat(&file_with(&[]), &no_phase),
        Err(SeatError::EmptySlot)
    );
}

#[test]
fn the_changed_slot_is_found() {
    let before = file_with(&[(0, slot_with(1)), (2, slot_with(2))]);
    assert_eq!(changed_slot(&before, &before), None);
    let mut after = before.clone();
    after[SLOTS_START + 2 * SLOT_BYTES + 0x300] ^= 1;
    assert_eq!(changed_slot(&before, &after), Some(2));
    after[SLOTS_START + 0x300] ^= 1;
    assert_eq!(changed_slot(&before, &after), None);
    let mut header_only = before.clone();
    header_only[0x10] ^= 1;
    assert_eq!(changed_slot(&before, &header_only), None);
    assert_eq!(changed_slot(&before[..10], &after), None);
}
