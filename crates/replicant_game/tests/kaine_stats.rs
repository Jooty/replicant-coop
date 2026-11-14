use replicant_game::GameMemory;
use replicant_game::kaine::{
    KAINE_LEVEL_SAVE_OFFSET, KAINE_PLAYER_FLAG_SAVE_OFFSET, LEVEL_STATS_ROW_BYTES,
    LEVEL_STATS_TABLE_RVA, clamped_current, level_row_in_range, param_patches, sync_kaine_stats,
};
use replicant_game::offsets::{
    ENTITY_HEALTH_OFFSET, ENTITY_LEVEL_OFFSET, ENTITY_MAGIC_OFFSET, ENTITY_RVA,
    PLAYER_PARAM_ARRAY_RVA,
};

use crate::fake_memory::{FAKE_MODULE_BASE, FakeMemory};

const TABLE_HEAP: u64 = 0x3_0000_0000;
const PROTAGONIST_LEVEL: i32 = 5;
const HER_LEVEL: i32 = 40;

fn row(
    max_health: i32,
    max_magic: f32,
    base_health: i32,
    base_magic: i32,
) -> [u8; LEVEL_STATS_ROW_BYTES] {
    let mut row = [0_u8; LEVEL_STATS_ROW_BYTES];
    row[4..8].copy_from_slice(&max_health.to_le_bytes());
    row[8..12].copy_from_slice(&max_magic.to_le_bytes());
    row[0x1C..0x20].copy_from_slice(&base_health.to_le_bytes());
    row[0x24..0x28].copy_from_slice(&base_magic.to_le_bytes());
    row
}

fn world(flag: i32) -> FakeMemory {
    let mut memory = FakeMemory::new();
    memory.put_at_rva(
        ENTITY_RVA + KAINE_PLAYER_FLAG_SAVE_OFFSET as u32,
        &flag.to_le_bytes(),
    );
    memory.put_at_rva(
        ENTITY_RVA + ENTITY_LEVEL_OFFSET,
        &PROTAGONIST_LEVEL.to_le_bytes(),
    );
    memory.put_at_rva(
        ENTITY_RVA + KAINE_LEVEL_SAVE_OFFSET as u32,
        &HER_LEVEL.to_le_bytes(),
    );
    memory.put_at_rva(ENTITY_RVA + ENTITY_HEALTH_OFFSET, &900_i32.to_le_bytes());
    memory.put_at_rva(ENTITY_RVA + ENTITY_MAGIC_OFFSET, &90.0_f32.to_le_bytes());
    memory.put_at_rva(LEVEL_STATS_TABLE_RVA, &TABLE_HEAP.to_le_bytes());
    let level_size = LEVEL_STATS_ROW_BYTES as u64;
    memory.put(
        TABLE_HEAP + PROTAGONIST_LEVEL as u64 * level_size,
        &row(300, 50.0, 30, 5),
    );
    memory.put(
        TABLE_HEAP + HER_LEVEL as u64 * level_size,
        &row(2000, 200.0, 200, 20),
    );
    memory.put_at_rva(PLAYER_PARAM_ARRAY_RVA, &[0; 0x68]);
    memory
}

#[test]
fn nothing_changes_while_she_is_not_the_player() {
    let mut memory = world(0);
    assert_eq!(sync_kaine_stats(&mut memory).unwrap(), None);
    assert_eq!(
        memory
            .read_i32(FAKE_MODULE_BASE + u64::from(ENTITY_RVA) + KAINE_LEVEL_SAVE_OFFSET as u64)
            .unwrap(),
        HER_LEVEL
    );
}

#[test]
fn her_maximums_follow_her_own_level_and_current_values_are_capped() {
    let mut memory = world(1);
    let synced = sync_kaine_stats(&mut memory).unwrap().unwrap();
    assert_eq!((synced.health, synced.max_health), (900, 2000));
    assert_eq!((synced.magic, synced.max_magic), (90.0, 200.0));
    assert_eq!(
        memory
            .read_i32(FAKE_MODULE_BASE + u64::from(PLAYER_PARAM_ARRAY_RVA) + 0x30)
            .unwrap(),
        200
    );
}

#[test]
fn the_first_bodys_level_is_never_copied_into_hers() {
    let mut memory = world(1);
    sync_kaine_stats(&mut memory).unwrap();
    assert_eq!(
        memory
            .read_i32(FAKE_MODULE_BASE + u64::from(ENTITY_RVA) + KAINE_LEVEL_SAVE_OFFSET as u64)
            .unwrap(),
        HER_LEVEL
    );
}

#[test]
fn once_in_step_nothing_is_written_or_logged_again() {
    let mut memory = world(1);
    assert!(sync_kaine_stats(&mut memory).unwrap().is_some());
    assert_eq!(sync_kaine_stats(&mut memory).unwrap(), None);
}

#[test]
fn her_own_level_up_is_followed_and_the_first_bodys_decides_nothing() {
    let mut memory = world(1);
    sync_kaine_stats(&mut memory).unwrap();
    memory.put_at_rva(ENTITY_RVA + ENTITY_LEVEL_OFFSET, &HER_LEVEL.to_le_bytes());
    assert_eq!(sync_kaine_stats(&mut memory).unwrap(), None);
    memory.put_at_rva(
        ENTITY_RVA + KAINE_LEVEL_SAVE_OFFSET as u32,
        &PROTAGONIST_LEVEL.to_le_bytes(),
    );
    let synced = sync_kaine_stats(&mut memory).unwrap().unwrap();
    assert_eq!(synced.max_health, 300);
}

#[test]
fn a_level_outside_the_table_is_left_alone() {
    assert!(!level_row_in_range(-1));
    assert!(!level_row_in_range(0x63));
    assert!(level_row_in_range(0x62));
}

#[test]
fn a_row_of_the_wrong_size_gives_no_patches() {
    assert!(param_patches(&[0; 8]).is_empty());
    assert_eq!(param_patches(&row(1, 2.0, 3, 4)).len(), 9);
}

#[test]
fn current_values_never_exceed_the_maximum() {
    assert_eq!(clamped_current(900, 300, 90.0, 50.0), (300, 50.0));
    assert_eq!(clamped_current(100, 300, 10.0, 50.0), (100, 10.0));
}
