use nc_launcher_core::{AssemblerStep, SeatAssembler, SeatSender, seat_checksum};
use replicant_game::story_progress::{SLOT_BYTES, fix_slot_checksum};
use replicant_proto::{SEAT_PART_BYTES, SeatPart, SeatRefusal};

const FIRST_TRANSFER: u32 = 4;
const SECOND_TRANSFER: u32 = 5;

pub fn generated_seat(seed: u8) -> Vec<u8> {
    let mut slot = vec![0_u8; SLOT_BYTES];
    slot[..4].copy_from_slice(&200_u32.to_le_bytes());
    slot[4] = b'P';
    for (index, byte) in slot[0x1000..0x8000].iter_mut().enumerate() {
        *byte = (index as u8).wrapping_mul(seed);
    }
    assert!(fix_slot_checksum(&mut slot));
    slot
}

pub fn all_parts(transfer: u32, seat: Vec<u8>) -> Vec<SeatPart> {
    let mut sender = SeatSender::new(transfer, seat).expect("a valid seat");
    sender.next_parts(usize::MAX)
}

fn feed_all(assembler: &mut SeatAssembler, parts: &[SeatPart]) -> AssemblerStep {
    parts
        .iter()
        .map(|part| assembler.accept(part))
        .last()
        .expect("at least one part")
}

#[test]
fn a_seat_is_cut_into_79_parts_of_480_bytes_and_a_rest() {
    let parts = all_parts(FIRST_TRANSFER, generated_seat(3));
    assert_eq!(parts.len(), 79);
    assert!(
        parts[..78]
            .iter()
            .all(|part| part.bytes.len() == SEAT_PART_BYTES)
    );
    assert_eq!(parts[78].bytes.len(), SLOT_BYTES - 78 * SEAT_PART_BYTES);
    assert!(
        parts
            .iter()
            .all(|part| part.parts == 79 && part.total_bytes == SLOT_BYTES as u32)
    );
}

#[test]
fn the_sender_hands_out_only_as_many_parts_as_asked() {
    let mut sender = SeatSender::new(FIRST_TRANSFER, generated_seat(3)).expect("valid");
    assert_eq!(sender.next_parts(8).len(), 8);
    assert_eq!(sender.parts_sent(), 8);
    assert!(!sender.is_all_sent());
    assert_eq!(sender.next_parts(usize::MAX).len(), 71);
    assert!(sender.is_all_sent());
    assert!(sender.next_parts(8).is_empty());
}

#[test]
fn the_sender_refuses_a_seat_that_is_not_one() {
    assert_eq!(
        SeatSender::new(FIRST_TRANSFER, vec![0; SLOT_BYTES]).err(),
        Some(SeatRefusal::NotASave)
    );
    assert_eq!(
        SeatSender::new(FIRST_TRANSFER, vec![0; 100]).err(),
        Some(SeatRefusal::WrongSize)
    );
}

#[test]
fn parts_in_order_give_the_seat_whole_at_the_last_one() {
    let seat = generated_seat(3);
    let parts = all_parts(FIRST_TRANSFER, seat.clone());
    let mut assembler = SeatAssembler::default();
    for part in &parts[..78] {
        assert!(matches!(
            assembler.accept(part),
            AssemblerStep::Progress { .. }
        ));
    }
    assert_eq!(assembler.accept(&parts[78]), AssemblerStep::Whole(seat));
}

#[test]
fn a_wrong_total_size_is_refused_at_the_first_part() {
    let mut parts = all_parts(FIRST_TRANSFER, generated_seat(3));
    parts[0].total_bytes = 40_000;
    let mut assembler = SeatAssembler::default();
    assert_eq!(
        assembler.accept(&parts[0]),
        AssemblerStep::Refused(SeatRefusal::WrongSize)
    );
}

#[test]
fn a_part_of_the_wrong_length_is_refused() {
    let mut parts = all_parts(FIRST_TRANSFER, generated_seat(3));
    parts[0].bytes.pop();
    let mut assembler = SeatAssembler::default();
    assert_eq!(
        assembler.accept(&parts[0]),
        AssemblerStep::Refused(SeatRefusal::WrongSize)
    );
}

#[test]
fn a_changed_byte_fails_the_checksum() {
    let mut parts = all_parts(FIRST_TRANSFER, generated_seat(3));
    parts[40].bytes[7] ^= 0xFF;
    let mut assembler = SeatAssembler::default();
    assert_eq!(
        feed_all(&mut assembler, &parts),
        AssemblerStep::Refused(SeatRefusal::BadChecksum)
    );
}

#[test]
fn bytes_that_are_not_a_save_are_refused_even_with_a_good_checksum() {
    let garbage = vec![9_u8; SLOT_BYTES];
    let mut parts = all_parts(FIRST_TRANSFER, generated_seat(3));
    for (index, part) in parts.iter_mut().enumerate() {
        let start = index * SEAT_PART_BYTES;
        part.bytes = garbage[start..start + part.bytes.len()].to_vec();
        part.checksum = seat_checksum(&garbage);
    }
    let mut assembler = SeatAssembler::default();
    assert_eq!(
        feed_all(&mut assembler, &parts),
        AssemblerStep::Refused(SeatRefusal::NotASave)
    );
}

#[test]
fn a_part_out_of_place_is_refused_and_keeps_nothing() {
    let seat = generated_seat(3);
    let parts = all_parts(FIRST_TRANSFER, seat);
    let mut assembler = SeatAssembler::default();
    assert!(matches!(
        assembler.accept(&parts[0]),
        AssemblerStep::Progress { .. }
    ));
    assert_eq!(
        assembler.accept(&parts[2]),
        AssemblerStep::Refused(SeatRefusal::OutOfOrder)
    );
    assert_eq!(assembler.accept(&parts[1]), AssemblerStep::Ignored);
    assert_eq!(assembler.accept(&parts[78]), AssemblerStep::Ignored);
}

#[test]
fn a_part_with_no_beginning_is_refused() {
    let parts = all_parts(FIRST_TRANSFER, generated_seat(3));
    let mut assembler = SeatAssembler::default();
    assert_eq!(
        assembler.accept(&parts[5]),
        AssemblerStep::Refused(SeatRefusal::OutOfOrder)
    );
}

#[test]
fn a_restarted_transfer_cannot_mix_with_the_old_one() {
    let old = generated_seat(3);
    let new = generated_seat(5);
    let old_parts = all_parts(FIRST_TRANSFER, old);
    let new_parts = all_parts(SECOND_TRANSFER, new.clone());
    let mut assembler = SeatAssembler::default();
    for part in &old_parts[..30] {
        assembler.accept(part);
    }
    assert_eq!(
        feed_all(&mut assembler, &new_parts),
        AssemblerStep::Whole(new)
    );
    assert_eq!(
        assembler.accept(&old_parts[30]),
        AssemblerStep::Refused(SeatRefusal::OutOfOrder)
    );
}

#[test]
fn a_part_from_the_middle_of_another_transfer_is_refused() {
    let first = all_parts(FIRST_TRANSFER, generated_seat(3));
    let second = all_parts(SECOND_TRANSFER, generated_seat(5));
    let mut assembler = SeatAssembler::default();
    assembler.accept(&first[0]);
    assembler.accept(&first[1]);
    assert_eq!(
        assembler.accept(&second[2]),
        AssemblerStep::Refused(SeatRefusal::OutOfOrder)
    );
}

#[test]
fn a_transfer_numbered_again_after_a_refusal_starts_clean() {
    let seat = generated_seat(3);
    let parts = all_parts(FIRST_TRANSFER, seat.clone());
    let mut assembler = SeatAssembler::default();
    assembler.accept(&parts[4]);
    assert_eq!(feed_all(&mut assembler, &parts), AssemblerStep::Whole(seat));
}
