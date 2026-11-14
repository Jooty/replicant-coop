use std::io;
use std::path::{Path, PathBuf};

use nc_launcher_core::{
    GuestSeatSync, GuestStartPlan, SeatSource, campaign_file_stem, plan_guest_start,
    seat_error_words, session_file_name,
};
use replicant_game::story_progress::{
    SAVE_FILE_BYTES, check_seat, seat_bytes_of, session_file_with_seat,
};

use crate::saves::write_atomic::{unix_seconds_now, unused_path, write_atomically};
use crate::saves::{SLOT_COUNT, fresh_coop_file, summarize_seat};

const SEAT_BACKUPS_FOLDER: &str = "seat-backups";
const HIDDEN_COPY_EXTENSION: &str = "bin";
const UNSENT_COPY_EXTENSION: &str = "unsent.bin";
const REPLACED_SEAT_EXTENSION: &str = "bin";

/// What the guest's launcher wrote before its game starts: the session file and the sync that
/// sends the seat back.
#[derive(Debug)]
pub struct PreparedSession {
    pub file_name: String,
    pub path: PathBuf,
    pub plan: GuestStartPlan,
    pub sync: GuestSeatSync,
    pub kept_aside: Vec<PathBuf>,
}

pub fn seat_backups_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(SEAT_BACKUPS_FOLDER)
}

pub fn hidden_copy_path(backups: &Path, campaign_id: &str) -> Option<PathBuf> {
    let stem = campaign_file_stem(campaign_id)?;
    Some(backups.join(format!("{stem}.{HIDDEN_COPY_EXTENSION}")))
}

pub fn unsent_copy_path(backups: &Path, campaign_id: &str) -> Option<PathBuf> {
    let stem = campaign_file_stem(campaign_id)?;
    Some(backups.join(format!("{stem}.{UNSENT_COPY_EXTENSION}")))
}

/// Keeps a seat that has still to reach the host, so a later session can send it.
pub fn keep_unsent(backups: &Path, campaign_id: &str, seat: &[u8]) -> io::Result<()> {
    let path = unsent_copy_path(backups, campaign_id).ok_or_else(unsafe_id)?;
    write_atomically(&path, seat)
}

pub fn read_unsent(backups: &Path, campaign_id: &str) -> Option<Vec<u8>> {
    read_valid_seat(&unsent_copy_path(backups, campaign_id)?)
}

pub fn read_hidden_copy(backups: &Path, campaign_id: &str) -> Option<Vec<u8>> {
    read_valid_seat(&hidden_copy_path(backups, campaign_id)?)
}

/// The host has `seat` whole: it becomes the hidden copy, and the unsent copy that held the same
/// bytes is the one moved there.
pub fn promote_delivered(backups: &Path, campaign_id: &str, seat: &[u8]) -> io::Result<()> {
    let hidden = hidden_copy_path(backups, campaign_id).ok_or_else(unsafe_id)?;
    let unsent = unsent_copy_path(backups, campaign_id).ok_or_else(unsafe_id)?;
    if std::fs::read(&unsent).is_ok_and(|bytes| bytes == seat) {
        return std::fs::rename(&unsent, &hidden);
    }
    write_atomically(&hidden, seat)
}

/// The newest save in a session file, by its own play time.
pub fn newest_seat_in(file: &[u8]) -> Option<Vec<u8>> {
    seats_in(file)
        .into_iter()
        .max_by(|a, b| play_seconds_of(a).total_cmp(&play_seconds_of(b)))
}

pub fn seats_in(file: &[u8]) -> Vec<Vec<u8>> {
    (0..SLOT_COUNT)
        .filter_map(|slot| seat_bytes_of(file, slot).ok())
        .collect()
}

pub fn play_seconds_of(seat: &[u8]) -> f64 {
    summarize_seat(seat).map_or(f64::NEG_INFINITY, |slot| slot.play_seconds)
}

/// Writes the guest's session file for a campaign from the newest of the host's seat, an unsent
/// copy and the session file already there. Any save in the old session file that is found nowhere
/// else is kept aside first.
pub fn prepare_session_file(
    save_folder: &Path,
    template: &[u8],
    backups: &Path,
    campaign_id: &str,
    from_host: &[u8],
) -> Result<PreparedSession, String> {
    let file_name = session_file_name(campaign_id)
        .ok_or_else(|| format!("the campaign id {campaign_id:?} cannot name a file"))?;
    check_seat(from_host).map_err(|error| {
        format!(
            "the host's save failed its checks: {}",
            seat_error_words(error)
        )
    })?;
    let path = save_folder.join(&file_name);
    let existing = std::fs::read(&path).ok();
    let session_seat = existing.as_deref().and_then(newest_seat_in);
    let unsent = read_unsent(backups, campaign_id);
    let hidden = read_hidden_copy(backups, campaign_id);
    let plan = plan_guest_start(
        play_seconds_of(from_host),
        unsent.as_deref().map(play_seconds_of),
        session_seat.as_deref().map(play_seconds_of),
    );
    let chosen = match plan.boot_from {
        SeatSource::Unsent => unsent.clone(),
        SeatSource::SessionFile => session_seat.clone(),
        SeatSource::FromHost => None,
    }
    .unwrap_or_else(|| from_host.to_vec());
    let blank = fresh_coop_file(Some(template)).map_err(|error| {
        format!("could not make a blank save file from this machine's saves: {error}")
    })?;
    let file = session_file_with_seat(&blank, &chosen).map_err(|error| {
        format!(
            "could not build the session file: {}",
            seat_error_words(error)
        )
    })?;
    let mut kept_aside = Vec::new();
    let found_elsewhere = |seat: &[u8]| {
        seat == chosen.as_slice()
            || [Some(from_host), unsent.as_deref(), hidden.as_deref()].contains(&Some(seat))
    };
    let stem = campaign_file_stem(campaign_id).unwrap_or("unknown");
    for old in existing.as_deref().map(seats_in).unwrap_or_default() {
        if found_elsewhere(&old) {
            continue;
        }
        let target = unused_path(
            backups,
            &format!("{stem}.replaced-{}", unix_seconds_now()),
            REPLACED_SEAT_EXTENSION,
        );
        write_atomically(&target, &old)
            .map_err(|error| format!("could not keep an older session save aside: {error}"))?;
        kept_aside.push(target);
    }
    if plan.send_back {
        keep_unsent(backups, campaign_id, &chosen)
            .map_err(|error| format!("could not keep the newer save for the host: {error}"))?;
    }
    if existing.as_deref() != Some(file.as_slice()) {
        write_atomically(&path, &file)
            .map_err(|error| format!("could not write the session file: {error}"))?;
    }
    let pending = plan.send_back.then(|| chosen.clone());
    Ok(PreparedSession {
        file_name,
        path,
        plan,
        sync: GuestSeatSync::new(
            campaign_id.to_owned(),
            file,
            Some(from_host.to_vec()),
            pending,
        ),
        kept_aside,
    })
}

pub fn read_session_file(path: &Path) -> io::Result<Vec<u8>> {
    let bytes = std::fs::read(path)?;
    if bytes.len() != SAVE_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("the session file has {} bytes", bytes.len()),
        ));
    }
    Ok(bytes)
}

fn read_valid_seat(path: &Path) -> Option<Vec<u8>> {
    let bytes = std::fs::read(path).ok()?;
    check_seat(&bytes).is_ok().then_some(bytes)
}

fn unsafe_id() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "the campaign id cannot name a file",
    )
}
