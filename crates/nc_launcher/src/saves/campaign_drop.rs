use std::io;
use std::path::{Path, PathBuf};

use crate::saves::SlotSummary;
use crate::saves::campaigns::{
    CampaignRecord, campaigns_path_beside, read_campaigns, render_campaigns, write_campaigns,
};
use crate::saves::seats::{seat_file_path, seats_dir_beside, set_aside_seats_dir, set_seat_aside};

const PLAY_SECONDS_TOLERANCE: f64 = 0.5;
const SET_ASIDE_RECORD_EXTENSION: &str = "toml";
const SET_ASIDE_SEAT_EXTENSION: &str = "bin";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetAside {
    pub host_slot: u8,
    pub record: Option<PathBuf>,
    pub seat: Option<PathBuf>,
}

/// Takes a host slot's campaign out of the record. Nothing is deleted: the entry and the seat move
/// to the set-aside folder beside the seats, named for the campaign id.
pub fn drop_campaign(coop_file: &Path, host_slot: u8) -> io::Result<SetAside> {
    let record_path = campaigns_path_beside(coop_file);
    let mut record = read_campaigns(&record_path);
    let seats = seats_dir_beside(coop_file);
    let entry = record.get(&host_slot).cloned();
    let mut moved = SetAside {
        host_slot,
        record: None,
        seat: None,
    };
    if entry.is_none() && !seat_file_path(&seats, host_slot).is_file() {
        return Ok(moved);
    }
    let aside = set_aside_seats_dir(&seats);
    std::fs::create_dir_all(&aside)?;
    let named = entry
        .as_ref()
        .map(|campaign| file_safe(&campaign.id))
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| format!("slot-{host_slot}"));
    let stem = unused_stem(&aside, &named);
    if let Some(campaign) = entry {
        let target = aside.join(format!("{stem}.{SET_ASIDE_RECORD_EXTENSION}"));
        let staging = target.with_extension("tmp");
        let single = CampaignRecord::from([(host_slot, campaign)]);
        std::fs::write(&staging, render_campaigns(&single))?;
        std::fs::rename(&staging, &target)?;
        moved.record = Some(target);
    }
    moved.seat = set_seat_aside(&seats, host_slot, &stem)?;
    if record.remove(&host_slot).is_some() {
        write_campaigns(&record_path, &record)?;
    }
    Ok(moved)
}

pub fn host_slots_that_lost_their_campaign(
    record: &CampaignRecord,
    slots: &[SlotSummary],
) -> Vec<u8> {
    record
        .iter()
        .filter(|(host_slot, campaign)| {
            let slot = slots
                .iter()
                .find(|slot| slot.index == **host_slot && slot.used);
            match slot {
                None => true,
                Some(slot) => {
                    slot.name.trim() != campaign.host_name.trim()
                        || slot.play_seconds + PLAY_SECONDS_TOLERANCE < campaign.host_play_seconds
                }
            }
        })
        .map(|(host_slot, _)| *host_slot)
        .collect()
}

pub fn drop_lost_campaigns(coop_file: &Path, slots: &[SlotSummary]) -> io::Result<Vec<SetAside>> {
    let record = read_campaigns(&campaigns_path_beside(coop_file));
    host_slots_that_lost_their_campaign(&record, slots)
        .into_iter()
        .map(|host_slot| drop_campaign(coop_file, host_slot))
        .collect()
}

fn file_safe(id: &str) -> String {
    id.chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .collect()
}

fn unused_stem(aside: &Path, named: &str) -> String {
    let taken = |stem: &str| {
        aside
            .join(format!("{stem}.{SET_ASIDE_RECORD_EXTENSION}"))
            .exists()
            || aside
                .join(format!("{stem}.{SET_ASIDE_SEAT_EXTENSION}"))
                .exists()
    };
    if !taken(named) {
        return named.to_owned();
    }
    (2_u32..)
        .map(|count| format!("{named}-{count}"))
        .find(|stem| !taken(stem))
        .unwrap_or_else(|| named.to_owned())
}
