use std::path::PathBuf;

use nc_launcher::saves::{
    Campaign, SeatFileError, SeatSummary, SlotSummary, backup_coop_file, coop_campaigns_path,
    coop_gamedata_path, coop_seats_dir, copy_slot_into_coop, drop_lost_campaigns, new_campaign_id,
    parse_campaigns, read_campaign_seat, read_campaigns, read_seat, render_campaigns,
    restore_coop_backup, stage_seat, store_campaign, store_campaign_seat, store_seat,
};
use nc_launcher_core::seat_checksum;
use replicant_game::story_progress::{fix_header_checksum, fix_slot_checksum};

use crate::scratch::fresh_dir;

const SLOTS_START: usize = 0x8160;
const SLOT_BYTES: usize = 0x9260;

fn campaign(id: &str, host_name: &str, host_seconds: f64) -> Campaign {
    Campaign {
        id: id.to_owned(),
        host_name: host_name.to_owned(),
        host_play_seconds: host_seconds,
        seat: Some(SeatSummary {
            name: "Guest".to_owned(),
            level: 7,
            play_seconds: 125.5,
        }),
        arrived_unix_seconds: Some(1_794_000_000),
        seat_checksum: None,
    }
}

fn seat_slot(seed: u8) -> Vec<u8> {
    let mut slot = vec![0_u8; SLOT_BYTES];
    slot[..4].copy_from_slice(&200_u32.to_le_bytes());
    slot[4] = b'P';
    slot[0x200] = seed;
    assert!(fix_slot_checksum(&mut slot));
    slot
}

fn normal_file(slot: usize, name: &str, seconds: f64) -> Vec<u8> {
    let mut bytes = vec![0_u8; 0x4_8200];
    for index in 0..7 {
        let start = SLOTS_START + index * SLOT_BYTES;
        bytes[start..start + 4].copy_from_slice(&200_u32.to_le_bytes());
    }
    let start = SLOTS_START + slot * SLOT_BYTES;
    bytes[start + 4..start + 5].copy_from_slice(b"P");
    bytes[start + 0x2C..start + 0x2C + name.len()].copy_from_slice(name.as_bytes());
    bytes[start + 0x4A0..start + 0x4A8].copy_from_slice(&seconds.to_le_bytes());
    for index in 0..7 {
        let at = SLOTS_START + index * SLOT_BYTES;
        fix_slot_checksum(&mut bytes[at..at + SLOT_BYTES]);
    }
    bytes[0x6C] = 1 << slot;
    fix_header_checksum(&mut bytes).unwrap();
    bytes
}

fn save_folder(name: &str, slot: usize, player: &str, seconds: f64) -> PathBuf {
    let saves = fresh_dir(name).join("My Games");
    let steam = saves.join("Steam/76561198000000000");
    std::fs::create_dir_all(&steam).unwrap();
    std::fs::write(steam.join("GAMEDATA"), normal_file(slot, player, seconds)).unwrap();
    saves
}

#[test]
fn a_campaign_record_round_trips() {
    let mut record = std::collections::BTreeMap::new();
    record.insert(
        2,
        campaign("0123456789abcdef0123456789abcdef", "Jay", 3725.0),
    );
    let mut empty_seat = campaign("fedcba9876543210fedcba9876543210", "Jay", 10.0);
    empty_seat.seat = None;
    empty_seat.arrived_unix_seconds = None;
    record.insert(5, empty_seat);
    let text = render_campaigns(&record);
    assert!(text.contains("[campaigns.2]"));
    assert!(text.contains("[campaigns.2.seat]"));
    assert_eq!(parse_campaigns(&text), record);
    assert!(parse_campaigns("not toml {{").is_empty());
    assert_eq!(new_campaign_id().len(), 32);
    assert_ne!(new_campaign_id(), new_campaign_id());
}

#[test]
fn a_backup_takes_the_seats_with_it() {
    let saves = save_folder("seat_backup", 0, "Jay", 100.0);
    let backups = fresh_dir("seat_backup_root");
    copy_slot_into_coop(&saves, 0, 0).unwrap();
    let record_path = coop_campaigns_path(&saves).unwrap();
    let seats = coop_seats_dir(&saves).unwrap();
    store_campaign(&record_path, 0, campaign("a", "Jay", 100.0)).unwrap();
    store_seat(&seats, 0, &seat_slot(1)).unwrap();
    let destination = backup_coop_file(&saves, &backups, "one").unwrap();
    assert!(destination.join("GAMEDATA.coop.campaigns.toml").is_file());
    assert!(destination.join("GAMEDATA.coop.seats/0.bin").is_file());

    store_campaign(&record_path, 0, campaign("b", "Jay", 100.0)).unwrap();
    store_seat(&seats, 0, &seat_slot(2)).unwrap();
    store_seat(&seats, 4, &seat_slot(3)).unwrap();
    restore_coop_backup(&saves, &backups, "one", "now").unwrap();
    assert_eq!(read_campaigns(&record_path)[&0].id, "a");
    assert_eq!(read_seat(&seats, 0, None).unwrap().unwrap(), seat_slot(1));
    assert!(read_seat(&seats, 4, None).unwrap().is_none());
    assert!(
        backups
            .join("before-restore-now/GAMEDATA.coop.seats/4.bin")
            .is_file()
    );
}

#[test]
fn overwriting_a_host_slot_drops_its_campaign() {
    let saves = save_folder("seat_overwrite", 0, "Jay", 100.0);
    copy_slot_into_coop(&saves, 0, 0).unwrap();
    copy_slot_into_coop(&saves, 0, 1).unwrap();
    let record_path = coop_campaigns_path(&saves).unwrap();
    let seats = coop_seats_dir(&saves).unwrap();
    store_campaign(&record_path, 0, campaign("a", "Jay", 100.0)).unwrap();
    store_campaign(&record_path, 1, campaign("b", "Jay", 100.0)).unwrap();
    store_seat(&seats, 0, &seat_slot(1)).unwrap();
    store_seat(&seats, 1, &seat_slot(2)).unwrap();
    copy_slot_into_coop(&saves, 0, 1).unwrap();
    let record = read_campaigns(&record_path);
    assert!(record.contains_key(&0) && !record.contains_key(&1));
    assert!(read_seat(&seats, 1, None).unwrap().is_none());
    assert!(read_seat(&seats, 0, None).unwrap().is_some());
    assert_eq!(
        std::fs::read(seats.join("orphaned/b.bin")).unwrap(),
        seat_slot(2)
    );
    assert_eq!(
        parse_campaigns(&std::fs::read_to_string(seats.join("orphaned/b.toml")).unwrap())[&1].id,
        "b"
    );
}

#[test]
fn a_deleted_or_replaced_host_slot_is_found() {
    let saves = save_folder("seat_deleted", 0, "Jay", 100.0);
    copy_slot_into_coop(&saves, 0, 0).unwrap();
    let coop = coop_gamedata_path(&saves).unwrap();
    let record_path = coop_campaigns_path(&saves).unwrap();
    let seats = coop_seats_dir(&saves).unwrap();
    for (slot, name, seconds) in [(0, "Jay", 100.0), (1, "Jay", 100.0), (2, "Jay", 900.0)] {
        store_campaign(&record_path, slot, campaign("x", name, seconds)).unwrap();
        store_seat(&seats, slot, &seat_slot(slot)).unwrap();
    }
    let summary = |index, used, name: &str, play_seconds| SlotSummary {
        index,
        used,
        name: name.to_owned(),
        phase: "P".to_owned(),
        level: 1,
        play_seconds,
    };
    let slots = [
        summary(0, true, "Jay", 160.0),
        summary(1, false, "", 0.0),
        summary(2, true, "Jay", 50.0),
    ];
    let dropped: Vec<u8> = drop_lost_campaigns(&coop, &slots)
        .unwrap()
        .iter()
        .map(|moved| moved.host_slot)
        .collect();
    assert_eq!(dropped, vec![1, 2]);
    assert_eq!(read_campaigns(&record_path).len(), 1);
    assert!(read_seat(&seats, 2, None).unwrap().is_none());
    assert!(read_seat(&seats, 0, None).unwrap().is_some());
}

#[test]
fn a_damaged_seat_is_not_stored_or_read() {
    let seats = fresh_dir("seat_damaged").join("seats");
    let mut damaged = seat_slot(1);
    damaged[0x300] ^= 0xFF;
    assert!(store_seat(&seats, 0, &damaged).is_err());
    store_seat(&seats, 0, &seat_slot(1)).unwrap();
    std::fs::write(seats.join("0.bin"), &damaged).unwrap();
    assert!(read_seat(&seats, 0, None).is_err());
}

#[test]
fn a_replaced_host_slot_moves_its_seat_aside() {
    let saves = save_folder("seat_replaced_aside", 0, "Jay", 100.0);
    copy_slot_into_coop(&saves, 0, 3).unwrap();
    let coop = coop_gamedata_path(&saves).unwrap();
    let record_path = coop_campaigns_path(&saves).unwrap();
    let seats = coop_seats_dir(&saves).unwrap();
    for id in ["kept", "kept"] {
        store_campaign(&record_path, 3, campaign(id, "Jay", 900.0)).unwrap();
        store_seat(&seats, 3, &seat_slot(9)).unwrap();
        let slots = [SlotSummary {
            index: 3,
            used: true,
            name: "Someone else".to_owned(),
            phase: "P".to_owned(),
            level: 1,
            play_seconds: 50.0,
        }];
        let moved = drop_lost_campaigns(&coop, &slots).unwrap();
        assert_eq!(moved.len(), 1);
        assert!(moved[0].seat.as_ref().is_some_and(|path| path.is_file()));
        assert!(moved[0].record.as_ref().is_some_and(|path| path.is_file()));
    }
    assert!(read_campaigns(&record_path).is_empty());
    assert!(read_seat(&seats, 3, None).unwrap().is_none());
    for name in ["kept.bin", "kept.toml", "kept-2.bin", "kept-2.toml"] {
        assert!(seats.join("orphaned").join(name).is_file(), "{name}");
    }
    assert_eq!(
        std::fs::read(seats.join("orphaned/kept-2.bin")).unwrap(),
        seat_slot(9)
    );
}

#[test]
fn a_stored_seat_is_checked_against_the_record() {
    let saves = save_folder("seat_record_checksum", 0, "Jay", 100.0);
    copy_slot_into_coop(&saves, 0, 2).unwrap();
    let coop = coop_gamedata_path(&saves).unwrap();
    let record_path = coop_campaigns_path(&saves).unwrap();
    let seats = coop_seats_dir(&saves).unwrap();
    store_campaign(&record_path, 2, campaign("c", "Jay", 100.0)).unwrap();
    let summary = SeatSummary {
        name: "Guest".to_owned(),
        level: 3,
        play_seconds: 40.0,
    };
    store_campaign_seat(&coop, 2, &seat_slot(4), summary.clone(), 7).unwrap();
    let entry = read_campaigns(&record_path)[&2].clone();
    assert_eq!(entry.seat_checksum, Some(seat_checksum(&seat_slot(4))));
    assert_eq!(entry.seat, Some(summary.clone()));
    assert_eq!(read_campaign_seat(&coop, 2).unwrap(), Some(seat_slot(4)));

    store_seat(&seats, 2, &seat_slot(5)).unwrap();
    assert!(matches!(
        read_campaign_seat(&coop, 2),
        Err(SeatFileError::ChecksumMismatch { .. })
    ));
    assert_eq!(std::fs::read(seats.join("2.bin")).unwrap(), seat_slot(5));

    store_seat(&seats, 2, &seat_slot(4)).unwrap();
    stage_seat(&seats, 2, &seat_slot(6)).unwrap();
    let mut record = read_campaigns(&record_path);
    record.get_mut(&2).unwrap().seat_checksum = Some(seat_checksum(&seat_slot(6)));
    nc_launcher::saves::write_campaigns(&record_path, &record).unwrap();
    assert_eq!(read_campaign_seat(&coop, 2).unwrap(), Some(seat_slot(6)));
    assert_eq!(std::fs::read(seats.join("2.bin")).unwrap(), seat_slot(6));
    assert!(store_campaign_seat(&coop, 5, &seat_slot(4), summary, 7).is_err());
}
