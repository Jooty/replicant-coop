use std::io;
use std::path::{Path, PathBuf};

use nc_launcher_core::{HostStore, campaign_file_stem, host_store_decision};
use replicant_game::story_progress::check_seat;
use replicant_proto::{CampaignCard, GUEST_BODY, SaveSummary, SecondSeatCard};

use crate::saves::campaigns::{campaigns_path_beside, read_campaigns, write_campaigns};
use crate::saves::seats::{seats_dir_beside, set_aside_seats_dir, set_seat_aside};
use crate::saves::write_atomic::{unused_path, write_atomically};
use crate::saves::{
    Campaign, SeatFileError, SeatSummary, SlotSummary, new_campaign_id, read_campaign_seat,
    store_campaign_seat, summarize_seat,
};

const SET_ASIDE_SEAT_EXTENSION: &str = "bin";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrivedSeat {
    Stored {
        unreadable_set_aside: Option<PathBuf>,
    },
    KeptAside {
        path: PathBuf,
        reason: String,
    },
}

/// The campaign of a host slot, made on first use with an empty seat; its host name and play time
/// follow the slot. A slot that is not in use has none.
pub fn campaign_for_host_slot(
    coop_file: &Path,
    slot: &SlotSummary,
) -> io::Result<Option<Campaign>> {
    if !slot.used {
        return Ok(None);
    }
    let record_path = campaigns_path_beside(coop_file);
    let mut record = read_campaigns(&record_path);
    let current = record.get(&slot.index).cloned();
    let campaign = match current.clone() {
        Some(mut campaign) => {
            campaign.host_name = slot.name.trim().to_owned();
            campaign.host_play_seconds = slot.play_seconds;
            campaign
        }
        None => Campaign {
            id: new_campaign_id(),
            host_name: slot.name.trim().to_owned(),
            host_play_seconds: slot.play_seconds,
            seat: None,
            arrived_unix_seconds: None,
            seat_checksum: None,
        },
    };
    if current.as_ref() != Some(&campaign) {
        record.insert(slot.index, campaign.clone());
        write_campaigns(&record_path, &record)?;
    }
    Ok(Some(campaign))
}

pub fn campaign_card_of(campaign: &Campaign, slot: &SlotSummary) -> CampaignCard {
    CampaignCard {
        campaign_id: campaign.id.clone(),
        host: SaveSummary {
            name: slot.name.trim().to_owned(),
            level: slot.level,
            play_seconds: slot.play_seconds,
        },
        second_seat: campaign.seat.as_ref().map(|seat| SecondSeatCard {
            summary: SaveSummary {
                name: seat.name.trim().to_owned(),
                level: seat.level,
                play_seconds: seat.play_seconds,
            },
            body: Some(GUEST_BODY),
        }),
    }
}

pub fn seat_summary_of(seat: &[u8]) -> Option<SeatSummary> {
    summarize_seat(seat).map(|slot| SeatSummary {
        name: slot.name.trim().to_owned(),
        level: slot.level,
        play_seconds: slot.play_seconds,
    })
}

/// A seat the guest sent back for the campaign being played. It replaces the stored one only when
/// it has more play time; any other copy is kept in the set-aside folder.
pub fn store_arrived_seat(
    coop_file: &Path,
    host_slot: u8,
    campaign_id: &str,
    seat: &[u8],
    now_unix_seconds: u64,
) -> Result<ArrivedSeat, SeatFileError> {
    check_seat(seat).map_err(SeatFileError::Damaged)?;
    let Some(summary) = seat_summary_of(seat) else {
        return Err(SeatFileError::Damaged(
            replicant_game::story_progress::SeatError::SlotSizeWrong,
        ));
    };
    let record = read_campaigns(&campaigns_path_beside(coop_file));
    let seats = seats_dir_beside(coop_file);
    let stem = campaign_file_stem(campaign_id).unwrap_or("unknown");
    let keep_aside = |reason: String| -> Result<ArrivedSeat, SeatFileError> {
        let path = keep_arrived_seat_aside(coop_file, stem, seat, now_unix_seconds)?;
        Ok(ArrivedSeat::KeptAside { path, reason })
    };
    if record
        .get(&host_slot)
        .is_none_or(|entry| entry.id != campaign_id)
    {
        return keep_aside(format!(
            "host slot {host_slot} no longer holds campaign {campaign_id}"
        ));
    };
    let (stored_play_seconds, unreadable_set_aside) = match read_campaign_seat(coop_file, host_slot)
    {
        Ok(stored) => (
            stored
                .as_deref()
                .and_then(seat_summary_of)
                .map(|stored| stored.play_seconds),
            None,
        ),
        Err(_) => (
            None,
            set_seat_aside(
                &seats,
                host_slot,
                &format!("{stem}-unreadable-{now_unix_seconds}"),
            )?,
        ),
    };
    match host_store_decision(stored_play_seconds, summary.play_seconds) {
        HostStore::Store => {
            store_campaign_seat(coop_file, host_slot, seat, summary, now_unix_seconds)?;
            Ok(ArrivedSeat::Stored {
                unreadable_set_aside,
            })
        }
        HostStore::SetAside => keep_aside(format!(
            "it has {:.0} s of play, the stored seat has {:.0} s",
            summary.play_seconds,
            stored_play_seconds.unwrap_or_default()
        )),
    }
}

/// Keeps a seat that arrived and is not stored, in the set-aside folder beside the seats.
pub fn keep_arrived_seat_aside(
    coop_file: &Path,
    stem: &str,
    seat: &[u8],
    now_unix_seconds: u64,
) -> io::Result<PathBuf> {
    let path = unused_path(
        &set_aside_seats_dir(&seats_dir_beside(coop_file)),
        &format!("{stem}-arrived-{now_unix_seconds}"),
        SET_ASIDE_SEAT_EXTENSION,
    );
    write_atomically(&path, seat)?;
    Ok(path)
}
