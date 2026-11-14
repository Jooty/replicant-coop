use crate::story_progress::{SAVE_FILE_BYTES, SAVE_FILE_SLOT_COUNT, SLOT_BYTES, SLOTS_FILE_OFFSET};

pub fn changed_slot(before: &[u8], after: &[u8]) -> Option<u8> {
    if before.len() != SAVE_FILE_BYTES || after.len() != SAVE_FILE_BYTES {
        return None;
    }
    let mut changed = (0..SAVE_FILE_SLOT_COUNT).filter(|&slot| {
        let range =
            SLOTS_FILE_OFFSET + slot * SLOT_BYTES..SLOTS_FILE_OFFSET + (slot + 1) * SLOT_BYTES;
        before[range.clone()] != after[range]
    });
    let first = changed.next()?;
    changed.next().is_none().then_some(first as u8)
}
