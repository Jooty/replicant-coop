use std::path::Path;

use crate::saves::SeatSummary;
use crate::saves::campaigns::{campaigns_path_beside, read_campaigns, write_campaigns};
use crate::saves::seats::{
    SeatFileError, finish_staged_seat, read_seat, seats_dir_beside, stage_seat,
};

/// Stores a seat for a host slot that has a campaign and records its summary and checksum. The
/// seat is staged first and moved into place only after the record names its checksum.
pub fn store_campaign_seat(
    coop_file: &Path,
    host_slot: u8,
    seat: &[u8],
    summary: SeatSummary,
    arrived_unix_seconds: u64,
) -> Result<(), SeatFileError> {
    let record_path = campaigns_path_beside(coop_file);
    let mut record = read_campaigns(&record_path);
    let Some(campaign) = record.get_mut(&host_slot) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("host slot {host_slot} has no campaign"),
        )
        .into());
    };
    let seats = seats_dir_beside(coop_file);
    let checksum = stage_seat(&seats, host_slot, seat)?;
    campaign.seat = Some(summary);
    campaign.seat_checksum = Some(checksum);
    campaign.arrived_unix_seconds = Some(arrived_unix_seconds);
    write_campaigns(&record_path, &record)?;
    finish_staged_seat(&seats, host_slot)?;
    Ok(())
}

pub fn read_campaign_seat(
    coop_file: &Path,
    host_slot: u8,
) -> Result<Option<Vec<u8>>, SeatFileError> {
    let record = read_campaigns(&campaigns_path_beside(coop_file));
    let expected = record
        .get(&host_slot)
        .and_then(|campaign| campaign.seat_checksum);
    read_seat(&seats_dir_beside(coop_file), host_slot, expected)
}
