use std::io;
use std::path::{Path, PathBuf};

use nc_launcher_core::seat_checksum;
use replicant_game::save_files::COOP_GAMEDATA_FILE_NAME;
use replicant_game::story_progress::{SeatError, check_seat};
use thiserror::Error;

use crate::saves::coop_gamedata_path;

const COOP_SEATS_SUFFIX: &str = ".seats";
const SEAT_FILE_EXTENSION: &str = "bin";
const SEAT_STAGING_EXTENSION: &str = "tmp";
const SET_ASIDE_SEATS_FOLDER: &str = "orphaned";

#[derive(Debug, Error)]
pub enum SeatFileError {
    #[error("the seat is damaged: {0:?}")]
    Damaged(SeatError),
    #[error(
        "the stored seat does not match the record's checksum (record {expected:08x}, file {found:08x})"
    )]
    ChecksumMismatch { expected: u32, found: u32 },
    #[error("{0}")]
    Io(#[from] io::Error),
}

pub fn coop_seats_dir(save_dir: &Path) -> Option<PathBuf> {
    let coop = coop_gamedata_path(save_dir)?;
    Some(seats_dir_beside(&coop))
}

pub(super) fn seats_dir_beside(coop_file: &Path) -> PathBuf {
    coop_file.with_file_name(format!("{COOP_GAMEDATA_FILE_NAME}{COOP_SEATS_SUFFIX}"))
}

pub fn seat_file_path(seats_dir: &Path, host_slot: u8) -> PathBuf {
    seats_dir.join(format!("{host_slot}.{SEAT_FILE_EXTENSION}"))
}

pub fn set_aside_seats_dir(seats_dir: &Path) -> PathBuf {
    seats_dir.join(SET_ASIDE_SEATS_FOLDER)
}

/// Writes the seat beside its final name and returns its checksum; `finish_staged_seat` moves it
/// into place. Until then a reader still finds the previous seat.
pub fn stage_seat(seats_dir: &Path, host_slot: u8, seat: &[u8]) -> Result<u32, SeatFileError> {
    check_seat(seat).map_err(SeatFileError::Damaged)?;
    std::fs::create_dir_all(seats_dir)?;
    std::fs::write(staged_seat_path(seats_dir, host_slot), seat)?;
    Ok(seat_checksum(seat))
}

pub fn finish_staged_seat(seats_dir: &Path, host_slot: u8) -> io::Result<()> {
    std::fs::rename(
        staged_seat_path(seats_dir, host_slot),
        seat_file_path(seats_dir, host_slot),
    )
}

pub fn store_seat(seats_dir: &Path, host_slot: u8, seat: &[u8]) -> Result<u32, SeatFileError> {
    let checksum = stage_seat(seats_dir, host_slot, seat)?;
    finish_staged_seat(seats_dir, host_slot)?;
    Ok(checksum)
}

/// Reads a stored seat. With the record's checksum, a seat that does not match is refused, unless
/// the staged copy matches: then a store was cut short after the record was written and is finished.
pub fn read_seat(
    seats_dir: &Path,
    host_slot: u8,
    expected_checksum: Option<u32>,
) -> Result<Option<Vec<u8>>, SeatFileError> {
    let stored = read_if_present(&seat_file_path(seats_dir, host_slot))?;
    let matches = |bytes: &[u8]| {
        check_seat(bytes).is_ok()
            && expected_checksum.is_none_or(|expected| seat_checksum(bytes) == expected)
    };
    if let Some(bytes) = &stored
        && matches(bytes)
    {
        return Ok(stored);
    }
    if expected_checksum.is_some()
        && let Some(staged) = read_if_present(&staged_seat_path(seats_dir, host_slot))?
        && matches(&staged)
    {
        finish_staged_seat(seats_dir, host_slot)?;
        return Ok(Some(staged));
    }
    let Some(bytes) = stored else {
        return Ok(None);
    };
    check_seat(&bytes).map_err(SeatFileError::Damaged)?;
    Err(SeatFileError::ChecksumMismatch {
        expected: expected_checksum.unwrap_or_default(),
        found: seat_checksum(&bytes),
    })
}

/// Moves a host slot's seat into the set-aside folder under `stem` and returns where it went.
pub fn set_seat_aside(seats_dir: &Path, host_slot: u8, stem: &str) -> io::Result<Option<PathBuf>> {
    let source = seat_file_path(seats_dir, host_slot);
    if !source.is_file() {
        return Ok(None);
    }
    let aside = set_aside_seats_dir(seats_dir);
    std::fs::create_dir_all(&aside)?;
    let target = aside.join(format!("{stem}.{SEAT_FILE_EXTENSION}"));
    std::fs::rename(&source, &target)?;
    Ok(Some(target))
}

fn staged_seat_path(seats_dir: &Path, host_slot: u8) -> PathBuf {
    seats_dir.join(format!("{host_slot}.{SEAT_STAGING_EXTENSION}"))
}

fn read_if_present(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}
