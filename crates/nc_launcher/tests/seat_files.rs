use nc_launcher::saves::{
    ArrivedSeat, SlotSummary, campaign_for_host_slot, coop_campaigns_path, coop_gamedata_path,
    copy_slot_into_coop, keep_unsent, prepare_session_file, promote_delivered, read_campaign_seat,
    read_campaigns, read_hidden_copy, read_unsent, seat_backups_dir, store_arrived_seat,
};
use nc_launcher_core::SeatSource;
use replicant_game::story_progress::{
    SLOT_BYTES, SLOTS_FILE_OFFSET, fix_header_checksum, fix_slot_checksum, seat_bytes_of,
};

use crate::scratch::fresh_dir;

const CAMPAIGN_ID: &str = "00c0ffee00c0ffee00c0ffee00c0ffee";

pub fn seat_with_play(name: &str, play_seconds: f64, seed: u8) -> Vec<u8> {
    let mut slot = vec![0_u8; SLOT_BYTES];
    slot[..4].copy_from_slice(&200_u32.to_le_bytes());
    slot[4] = b'P';
    slot[0x2C..0x2C + name.len()].copy_from_slice(name.as_bytes());
    slot[0x4A0..0x4A8].copy_from_slice(&play_seconds.to_le_bytes());
    slot[0x300] = seed;
    assert!(fix_slot_checksum(&mut slot));
    slot
}

pub fn template_file() -> Vec<u8> {
    let mut bytes = vec![0_u8; 0x4_8200];
    for index in 0..7 {
        let start = SLOTS_FILE_OFFSET + index * SLOT_BYTES;
        bytes[start..start + 4].copy_from_slice(&200_u32.to_le_bytes());
        fix_slot_checksum(&mut bytes[start..start + SLOT_BYTES]);
    }
    fix_header_checksum(&mut bytes).unwrap();
    bytes
}

fn host_saves(name: &str, player: &str, seconds: f64) -> std::path::PathBuf {
    let saves = fresh_dir(name).join("My Games");
    let steam = saves.join("Steam/76561198000000000");
    std::fs::create_dir_all(&steam).unwrap();
    let mut file = template_file();
    let start = SLOTS_FILE_OFFSET;
    file[start..start + SLOT_BYTES].copy_from_slice(&seat_with_play(player, seconds, 1));
    file[0x6C] = 1;
    fix_header_checksum(&mut file).unwrap();
    std::fs::write(steam.join("GAMEDATA"), file).unwrap();
    copy_slot_into_coop(&saves, 0, 0).unwrap();
    saves
}

fn used_slot(index: u8, name: &str, play_seconds: f64) -> SlotSummary {
    SlotSummary {
        index,
        used: true,
        name: name.to_owned(),
        phase: "P".to_owned(),
        level: 4,
        play_seconds,
    }
}

#[test]
fn an_old_slot_gets_a_campaign_on_first_use() {
    let saves = host_saves("old_slot_campaign", "Jay", 100.0);
    let coop = coop_gamedata_path(&saves).unwrap();
    let record_path = coop_campaigns_path(&saves).unwrap();
    assert!(read_campaigns(&record_path).is_empty());
    let first = campaign_for_host_slot(&coop, &used_slot(0, "Jay", 100.0))
        .unwrap()
        .expect("a campaign");
    assert_eq!(first.id.len(), 32);
    assert_eq!(first.seat, None);
    let again = campaign_for_host_slot(&coop, &used_slot(0, "Jay", 160.0))
        .unwrap()
        .expect("the same campaign");
    assert_eq!(again.id, first.id);
    assert_eq!(read_campaigns(&record_path)[&0].host_play_seconds, 160.0);
    let mut empty = used_slot(1, "", 0.0);
    empty.used = false;
    assert!(campaign_for_host_slot(&coop, &empty).unwrap().is_none());
}

#[test]
fn a_seat_back_at_the_host_replaces_only_an_older_one() {
    let saves = host_saves("seat_back_at_host", "Jay", 100.0);
    let coop = coop_gamedata_path(&saves).unwrap();
    let campaign = campaign_for_host_slot(&coop, &used_slot(0, "Jay", 100.0))
        .unwrap()
        .unwrap();
    let first = seat_with_play("Guest", 50.0, 1);
    assert_eq!(
        store_arrived_seat(&coop, 0, &campaign.id, &first, 10).unwrap(),
        ArrivedSeat::Stored {
            unreadable_set_aside: None
        }
    );
    let newer = seat_with_play("Guest", 80.0, 2);
    assert!(matches!(
        store_arrived_seat(&coop, 0, &campaign.id, &newer, 11).unwrap(),
        ArrivedSeat::Stored { .. }
    ));
    let entry = read_campaigns(&coop_campaigns_path(&saves).unwrap())[&0].clone();
    assert_eq!(entry.seat.map(|seat| seat.play_seconds), Some(80.0));
    assert_eq!(entry.arrived_unix_seconds, Some(11));
    let older = seat_with_play("Guest", 60.0, 3);
    let ArrivedSeat::KeptAside { path, .. } =
        store_arrived_seat(&coop, 0, &campaign.id, &older, 12).unwrap()
    else {
        panic!("an older seat must not replace a newer one");
    };
    assert_eq!(std::fs::read(path).unwrap(), older);
    let ArrivedSeat::KeptAside { path, reason } =
        store_arrived_seat(&coop, 0, "ffffffffffffffffffffffffffffffff", &newer, 13).unwrap()
    else {
        panic!("a seat for another campaign must not be stored");
    };
    assert!(path.is_file() && reason.contains("no longer holds"));
    let mut damaged = seat_with_play("Guest", 999.0, 4);
    damaged[0x310] ^= 0xFF;
    assert!(store_arrived_seat(&coop, 0, &campaign.id, &damaged, 14).is_err());
    assert_eq!(read_campaign_seat(&coop, 0).unwrap(), Some(newer));
}

#[test]
fn the_guests_session_file_holds_the_hosts_seat_at_slot_0() {
    let root = fresh_dir("guest_session_file");
    let folder = root.join("saves");
    std::fs::create_dir_all(&folder).unwrap();
    let backups = seat_backups_dir(&root.join("data"));
    let from_host = seat_with_play("Guest", 70.0, 1);
    let prepared =
        prepare_session_file(&folder, &template_file(), &backups, CAMPAIGN_ID, &from_host).unwrap();
    assert_eq!(prepared.file_name, "GAMEDATA.seat-00c0ffee");
    assert_eq!(prepared.plan.boot_from, SeatSource::FromHost);
    assert!(!prepared.plan.send_back);
    let written = std::fs::read(folder.join("GAMEDATA.seat-00c0ffee")).unwrap();
    assert_eq!(seat_bytes_of(&written, 0).unwrap(), from_host);
    assert_eq!(written[0x6C] & 0x7F, 1);
    assert!(read_unsent(&backups, CAMPAIGN_ID).is_none());
    assert!(prepare_session_file(&folder, &template_file(), &backups, "../x", &from_host).is_err());
}

#[test]
fn a_newer_local_copy_wins_and_an_older_unique_one_is_kept_aside() {
    let root = fresh_dir("guest_newer_local");
    let folder = root.join("saves");
    std::fs::create_dir_all(&folder).unwrap();
    let backups = seat_backups_dir(&root.join("data"));
    let unsent = seat_with_play("Guest", 150.0, 2);
    keep_unsent(&backups, CAMPAIGN_ID, &unsent).unwrap();
    let from_host = seat_with_play("Guest", 100.0, 1);
    let prepared =
        prepare_session_file(&folder, &template_file(), &backups, CAMPAIGN_ID, &from_host).unwrap();
    assert_eq!(prepared.plan.boot_from, SeatSource::Unsent);
    assert!(prepared.plan.send_back);
    assert_eq!(prepared.sync.to_send(), Some(unsent.clone()));
    assert_eq!(
        seat_bytes_of(&std::fs::read(&prepared.path).unwrap(), 0).unwrap(),
        unsent
    );

    promote_delivered(&backups, CAMPAIGN_ID, &unsent).unwrap();
    assert!(read_unsent(&backups, CAMPAIGN_ID).is_none());
    assert_eq!(
        read_hidden_copy(&backups, CAMPAIGN_ID),
        Some(unsent.clone())
    );

    let mut file = std::fs::read(&prepared.path).unwrap();
    let other = seat_with_play("Guest", 20.0, 9);
    let at = SLOTS_FILE_OFFSET + 3 * SLOT_BYTES;
    file[at..at + SLOT_BYTES].copy_from_slice(&other);
    std::fs::write(&prepared.path, &file).unwrap();
    let newest_host = seat_with_play("Guest", 400.0, 5);
    let again = prepare_session_file(
        &folder,
        &template_file(),
        &backups,
        CAMPAIGN_ID,
        &newest_host,
    )
    .unwrap();
    assert_eq!(again.plan.boot_from, SeatSource::FromHost);
    assert_eq!(again.kept_aside.len(), 1);
    assert_eq!(std::fs::read(&again.kept_aside[0]).unwrap(), other);
    assert_eq!(
        seat_bytes_of(&std::fs::read(&again.path).unwrap(), 0).unwrap(),
        newest_host
    );
}
