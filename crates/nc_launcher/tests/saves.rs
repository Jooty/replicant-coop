use chrono::{TimeZone, Utc};
use nc_launcher::saves::{BackupError, backup_saves, format_stamp, stamp_label};

use crate::scratch::fresh_dir;

#[test]
fn backup_copies_every_file_into_a_stamped_folder() {
    let base = fresh_dir("backup_copy");
    let saves = base.join("saves");
    std::fs::create_dir_all(saves.join("slot")).unwrap();
    std::fs::write(saves.join("a.dat"), b"a").unwrap();
    std::fs::write(saves.join("slot/b.dat"), b"b").unwrap();
    let (destination, files) =
        backup_saves(&saves, &base.join("backups"), "20261106-120000").unwrap();
    assert_eq!(files, 2);
    assert_eq!(std::fs::read(destination.join("slot/b.dat")).unwrap(), b"b");
}

#[test]
fn backup_refuses_to_overwrite_and_reports_a_missing_save_folder() {
    let base = fresh_dir("backup_refuse");
    let saves = base.join("saves");
    std::fs::create_dir_all(&saves).unwrap();
    backup_saves(&saves, &base.join("backups"), "s").unwrap();
    assert!(matches!(
        backup_saves(&saves, &base.join("backups"), "s"),
        Err(BackupError::AlreadyExists(_))
    ));
    assert!(matches!(
        backup_saves(&base.join("nope"), &base.join("backups"), "t"),
        Err(BackupError::NoSaveFolder(_))
    ));
}

#[test]
fn stamp_has_the_same_shape_as_the_shell_script() {
    let moment = Utc.with_ymd_and_hms(2026, 11, 6, 9, 5, 3).unwrap();
    assert_eq!(format_stamp(&moment), "20261106-090503");
}

fn gamedata_with_slot(slot: usize, name: &str, phase: &str, level: i32, seconds: f64) -> Vec<u8> {
    let mut bytes = vec![0u8; nc_launcher::saves::GAMEDATA_SIZE];
    for index in 0..nc_launcher::saves::SLOT_COUNT as usize {
        let start = nc_launcher::saves::SLOTS_START + index * nc_launcher::saves::SLOT_STRIDE;
        bytes[start..start + 4].copy_from_slice(&200u32.to_le_bytes());
    }
    let start = nc_launcher::saves::SLOTS_START + slot * nc_launcher::saves::SLOT_STRIDE;
    bytes[start + 4..start + 4 + phase.len()].copy_from_slice(phase.as_bytes());
    bytes[start + 0x2C..start + 0x2C + name.len()].copy_from_slice(name.as_bytes());
    bytes[start + 0x64..start + 0x68].copy_from_slice(&level.to_le_bytes());
    bytes[start + 0x4A0..start + 0x4A8].copy_from_slice(&seconds.to_le_bytes());
    bytes
}

#[test]
fn only_a_slot_with_a_phase_counts_as_used() {
    let bytes = gamedata_with_slot(2, "Jay", "A_CENTER_VILLAGE_01", 12, 3725.0);
    let slots = nc_launcher::saves::parse_slots(&bytes).unwrap();
    assert_eq!(slots.len(), 7);
    assert_eq!(slots.iter().filter(|slot| slot.used).count(), 1);
    assert_eq!(slots[2].name, "Jay");
    assert_eq!(slots[2].level, 12);
    assert_eq!(slots[2].play_time_text(), "1:02:05");
}

#[test]
fn a_save_list_reads_play_time_as_hours_and_minutes() {
    let bytes = gamedata_with_slot(2, "Jay", "A_CENTER_VILLAGE_01", 12, 80_000.0);
    let slots = nc_launcher::saves::parse_slots(&bytes).unwrap();
    assert_eq!(slots[2].play_time_short(), "22 h 13");
}

#[test]
fn a_backup_reads_as_its_day_and_time() {
    assert_eq!(stamp_label("20261112-134000"), "12 Nov, 13:40");
    assert_eq!(stamp_label("not-a-stamp"), "not-a-stamp");
}

#[test]
fn the_player_name_is_the_name_in_the_booted_slot() {
    let bytes = gamedata_with_slot(2, "Jay", "A_CENTER_VILLAGE_01", 12, 3725.0);
    let slots = nc_launcher::saves::parse_slots(&bytes).unwrap();
    assert_eq!(
        nc_launcher::saves::slot_player_name(&slots, Some(2)).as_deref(),
        Some("Jay")
    );
}

#[test]
fn an_unused_or_unselected_slot_gives_no_player_name() {
    let bytes = gamedata_with_slot(2, "Jay", "A_CENTER_VILLAGE_01", 12, 3725.0);
    let slots = nc_launcher::saves::parse_slots(&bytes).unwrap();
    assert_eq!(nc_launcher::saves::slot_player_name(&slots, Some(3)), None);
    assert_eq!(nc_launcher::saves::slot_player_name(&slots, None), None);
}

#[test]
fn the_default_slot_is_the_used_one_with_most_play_time() {
    let mut bytes = gamedata_with_slot(1, "A", "P", 1, 100.0);
    let other = gamedata_with_slot(4, "B", "P", 1, 900.0);
    let start = nc_launcher::saves::SLOTS_START + 4 * nc_launcher::saves::SLOT_STRIDE;
    let end = start + nc_launcher::saves::SLOT_STRIDE;
    bytes[start..end].copy_from_slice(&other[start..end]);
    let slots = nc_launcher::saves::parse_slots(&bytes).unwrap();
    assert_eq!(nc_launcher::saves::default_slot(&slots), Some(4));
    let empty =
        nc_launcher::saves::parse_slots(&vec![0u8; nc_launcher::saves::GAMEDATA_SIZE]).unwrap();
    assert_eq!(nc_launcher::saves::default_slot(&empty), None);
}

#[test]
fn a_file_of_the_wrong_size_is_refused() {
    assert!(matches!(
        nc_launcher::saves::parse_slots(&[0u8; 10]),
        Err(nc_launcher::saves::SlotError::WrongSize(10))
    ));
}

#[test]
fn restoring_a_backup_first_keeps_the_current_saves() {
    let base = fresh_dir("restore_keeps");
    let saves = base.join("saves");
    let backups = base.join("backups");
    std::fs::create_dir_all(&saves).unwrap();
    std::fs::write(saves.join("GAMEDATA"), b"old").unwrap();
    backup_saves(&saves, &backups, "20261106-100000").unwrap();
    std::fs::write(saves.join("GAMEDATA"), b"new").unwrap();
    nc_launcher::saves::restore_backup(&saves, &backups, "20261106-100000", "20261106-110000")
        .unwrap();
    assert_eq!(std::fs::read(saves.join("GAMEDATA")).unwrap(), b"old");
    let kept = backups.join("before-restore-20261106-110000/GAMEDATA");
    assert_eq!(std::fs::read(kept).unwrap(), b"new");
    assert_eq!(
        nc_launcher::saves::list_backups(&backups)[0],
        "before-restore-20261106-110000"
    );
}

fn save_folder_with_normal_slot(name: &str, slot: usize) -> std::path::PathBuf {
    let saves = fresh_dir(name);
    let user = saves.join("Steam").join("76561198000000000");
    std::fs::create_dir_all(&user).unwrap();
    let bytes = gamedata_with_slot(slot, "Jay", "A_CENTER_VILLAGE_01", 12, 3725.0);
    std::fs::write(user.join("GAMEDATA"), bytes).unwrap();
    saves
}

#[test]
fn copying_a_slot_creates_the_coop_file_and_leaves_the_normal_file_alone() {
    let saves = save_folder_with_normal_slot("coop_copy", 2);
    let normal = saves.join("Steam/76561198000000000/GAMEDATA");
    let before = std::fs::read(&normal).unwrap();
    let coop = nc_launcher::saves::copy_slot_into_coop(&saves, 2, 0).unwrap();
    assert!(coop.ends_with("GAMEDATA.coop"));
    assert_eq!(std::fs::read(&normal).unwrap(), before);
    let slots = nc_launcher::saves::read_coop_slots(&saves).unwrap();
    assert_eq!(slots.iter().filter(|slot| slot.used).count(), 1);
    assert_eq!(slots[0].name, "Jay");
    assert_eq!(nc_launcher::saves::first_empty_slot(&slots), Some(1));
}

#[test]
fn a_missing_coop_file_reads_as_not_found() {
    let saves = save_folder_with_normal_slot("coop_missing", 0);
    assert!(matches!(
        nc_launcher::saves::read_coop_slots(&saves),
        Err(nc_launcher::saves::SlotError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound
    ));
}

#[test]
fn coop_backup_and_restore_round_trip_with_a_safety_copy() {
    let saves = save_folder_with_normal_slot("coop_backup", 0);
    let backups = fresh_dir("coop_backup_root");
    nc_launcher::saves::copy_slot_into_coop(&saves, 0, 0).unwrap();
    let coop = saves.join("Steam/76561198000000000/GAMEDATA.coop");
    let original = std::fs::read(&coop).unwrap();
    nc_launcher::saves::backup_coop_file(&saves, &backups, "one").unwrap();
    std::fs::write(&coop, b"changed").unwrap();
    nc_launcher::saves::restore_coop_backup(&saves, &backups, "one", "now").unwrap();
    assert_eq!(std::fs::read(&coop).unwrap(), original);
    assert!(backups.join("before-restore-now/GAMEDATA.coop").is_file());
}

#[test]
fn an_unreadable_coop_file_is_never_copied_over() {
    let saves = save_folder_with_normal_slot("coop_target_bad", 0);
    let coop = saves.join("Steam/76561198000000000/GAMEDATA.coop");
    std::fs::write(&coop, b"short").unwrap();
    assert!(matches!(
        nc_launcher::saves::coop_copy_target_slot(&saves),
        Err(nc_launcher::saves::CoopTargetError::Unreadable(_))
    ));
}

#[test]
fn the_copy_target_is_slot_zero_only_when_there_is_no_coop_file() {
    let saves = save_folder_with_normal_slot("coop_target_new", 0);
    assert_eq!(
        nc_launcher::saves::coop_copy_target_slot(&saves).unwrap(),
        0
    );
    nc_launcher::saves::copy_slot_into_coop(&saves, 0, 0).unwrap();
    assert_eq!(
        nc_launcher::saves::coop_copy_target_slot(&saves).unwrap(),
        1
    );
}

#[test]
fn restoring_a_wrong_sized_backup_is_refused_and_changes_nothing() {
    let saves = save_folder_with_normal_slot("coop_restore_bad", 0);
    let backups = fresh_dir("coop_restore_bad_root");
    nc_launcher::saves::copy_slot_into_coop(&saves, 0, 0).unwrap();
    let coop = saves.join("Steam/76561198000000000/GAMEDATA.coop");
    let original = std::fs::read(&coop).unwrap();
    std::fs::create_dir_all(backups.join("bad")).unwrap();
    std::fs::write(backups.join("bad/GAMEDATA.coop"), b"tiny").unwrap();
    assert!(matches!(
        nc_launcher::saves::restore_coop_backup(&saves, &backups, "bad", "now"),
        Err(nc_launcher::saves::BackupError::WrongSize(4))
    ));
    assert_eq!(std::fs::read(&coop).unwrap(), original);
}

#[test]
fn two_steam_folders_without_a_game_file_are_not_guessed() {
    let saves = fresh_dir("coop_two_folders");
    std::fs::create_dir_all(saves.join("Steam/1")).unwrap();
    std::fs::create_dir_all(saves.join("Steam/2")).unwrap();
    assert_eq!(nc_launcher::saves::coop_gamedata_path(&saves), None);
}

fn vanilla_file_with_used_slot(slot: usize) -> Vec<u8> {
    let mut bytes = gamedata_with_slot(slot, "Jay", "A_CENTER_VILLAGE_01", 12, 3725.0);
    for index in 0..nc_launcher::saves::SLOT_COUNT as usize {
        let start = nc_launcher::saves::SLOTS_START + index * nc_launcher::saves::SLOT_STRIDE;
        replicant_game::story_progress::fix_slot_checksum(&mut bytes[start..start + 0x9260]);
    }
    for (index, byte) in bytes[..0x8154].iter_mut().enumerate() {
        *byte = (index % 251) as u8 | 1;
    }
    bytes[nc_launcher::saves::HEADER_SLOT_MASK_OFFSET] = 1 << slot;
    replicant_game::story_progress::fix_header_checksum(&mut bytes).unwrap();
    bytes
}

fn header_checksum_is_right(file: &[u8]) -> bool {
    let stored = u32::from_le_bytes(file[0x8154..0x8158].try_into().unwrap());
    file[..0x8154]
        .iter()
        .map(|&byte| u32::from(byte))
        .sum::<u32>()
        == stored
}

#[test]
fn a_merged_file_has_the_target_bit_and_a_valid_header_checksum() {
    let vanilla = vanilla_file_with_used_slot(2);
    let merged = nc_launcher::saves::merge_slot(&vanilla, None, 2, 4).unwrap();
    assert_ne!(
        merged[nc_launcher::saves::HEADER_SLOT_MASK_OFFSET] & (1 << 4),
        0
    );
    assert!(header_checksum_is_right(&merged));
    assert!(replicant_game::story_progress::save_file_is_valid(&merged));
    let again = nc_launcher::saves::merge_slot(&vanilla, Some(merged), 2, 1).unwrap();
    let mask = again[nc_launcher::saves::HEADER_SLOT_MASK_OFFSET];
    assert_eq!(mask & 0b1_0010, 0b1_0010);
    assert!(header_checksum_is_right(&again));
}

#[test]
fn a_fresh_file_is_valid_empty_and_keeps_the_rest_of_the_vanilla_header() {
    let vanilla = vanilla_file_with_used_slot(2);
    let fresh = nc_launcher::saves::fresh_coop_file(Some(&vanilla)).unwrap();
    assert_eq!(fresh.len(), nc_launcher::saves::GAMEDATA_SIZE);
    assert_eq!(fresh[nc_launcher::saves::HEADER_SLOT_MASK_OFFSET], 0);
    assert!(header_checksum_is_right(&fresh));
    assert!(
        nc_launcher::saves::parse_slots(&fresh)
            .unwrap()
            .iter()
            .all(|slot| !slot.used)
    );
    let ranges = &nc_launcher::saves::HEADER_ENDING_RECORD_RANGES;
    for index in 0..0x8154 {
        let in_ending_range = ranges.iter().any(|range| range.contains(&index));
        if index == nc_launcher::saves::HEADER_SLOT_MASK_OFFSET {
            continue;
        }
        if in_ending_range {
            assert_eq!(fresh[index], 0, "ending record byte {index:#x}");
        } else {
            assert_eq!(fresh[index], vanilla[index], "header byte {index:#x}");
        }
    }
}

#[test]
fn a_fresh_file_drops_exactly_the_ending_records() {
    use replicant_game::story_progress::{
        ENDING_ACCOMPLISHED_HEADER_OFFSET, ENDING_CLEAR_BITS_HEADER_OFFSET,
        ENDING_COSTUMES_HEADER_OFFSET,
    };
    let ranges = &nc_launcher::saves::HEADER_ENDING_RECORD_RANGES;
    assert_eq!(
        ranges[0],
        ENDING_CLEAR_BITS_HEADER_OFFSET..ENDING_CLEAR_BITS_HEADER_OFFSET + 4
    );
    assert_eq!(
        ranges[1],
        ENDING_COSTUMES_HEADER_OFFSET..ENDING_COSTUMES_HEADER_OFFSET + 7
    );
    assert_eq!(
        ranges[2],
        ENDING_ACCOMPLISHED_HEADER_OFFSET..ENDING_ACCOMPLISHED_HEADER_OFFSET + 0x2F
    );
    let fresh = nc_launcher::saves::fresh_coop_file(Some(&vanilla_file_with_used_slot(0))).unwrap();
    let records = replicant_game::story_progress::read_ending_records(&fresh).unwrap();
    assert!(replicant_game::story_progress::ending_records_are_empty(
        &records
    ));
}

#[test]
fn a_fresh_file_needs_a_normal_file_of_the_right_size() {
    assert!(nc_launcher::saves::fresh_coop_file(None).is_err());
    assert!(matches!(
        nc_launcher::saves::fresh_coop_file(Some(&[0u8; 10])),
        Err(nc_launcher::saves::SlotError::WrongSize(10))
    ));
}

#[test]
fn a_slot_is_shown_counting_from_one() {
    assert_eq!(nc_launcher::saves::slot_label(0), "01");
    assert_eq!(nc_launcher::saves::slot_label(1), "02");
    assert_eq!(nc_launcher::saves::slot_label(9), "10");
}
