use std::path::{Path, PathBuf};

use replicant_game::save_files::COOP_GAMEDATA_FILE_NAME;
use replicant_game::story_progress::fix_header_checksum;
use thiserror::Error;

use crate::saves::campaign_backup::{copy_companions_into, restore_companions_from};
use crate::saves::campaign_drop::drop_campaign;
use crate::saves::{
    BackupError, GAMEDATA_SIZE, HEADER_SLOT_MASK_OFFSET, SLOT_COUNT, SLOT_STRIDE, SLOTS_START,
    STEAM_SAVE_FOLDER, SlotError, SlotSummary, gamedata_path, parse_slots,
};

pub const COOP_BACKUP_FOLDER: &str = "coop-save-backups";
pub const COOP_BEFORE_RESTORE_PREFIX: &str = "before-restore-";

#[derive(Debug, Error)]
pub enum CoopTargetError {
    #[error("the co-op save file cannot be read ({0}); not copying over it")]
    Unreadable(SlotError),
    #[error("the co-op save file has no empty slot left")]
    Full,
}

pub fn coop_gamedata_path(save_dir: &Path) -> Option<PathBuf> {
    if let Some(vanilla) = gamedata_path(save_dir) {
        return Some(vanilla.with_file_name(COOP_GAMEDATA_FILE_NAME));
    }
    let folders: Vec<PathBuf> = std::fs::read_dir(save_dir.join(STEAM_SAVE_FOLDER))
        .ok()?
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| path.is_dir())
        .collect();
    let with_coop_file = folders
        .iter()
        .find(|folder| folder.join(COOP_GAMEDATA_FILE_NAME).is_file());
    match (with_coop_file, folders.as_slice()) {
        (Some(folder), _) | (None, [folder]) => Some(folder.join(COOP_GAMEDATA_FILE_NAME)),
        _ => None,
    }
}

pub fn coop_copy_target_slot(save_dir: &Path) -> Result<u8, CoopTargetError> {
    match read_coop_slots(save_dir) {
        Ok(slots) => first_empty_slot(&slots).ok_or(CoopTargetError::Full),
        Err(SlotError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(CoopTargetError::Unreadable(error)),
    }
}

pub fn read_coop_slots(save_dir: &Path) -> Result<Vec<SlotSummary>, SlotError> {
    let path = coop_gamedata_path(save_dir)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no save folder found"))?;
    parse_slots(&std::fs::read(path)?)
}

pub fn first_empty_slot(slots: &[SlotSummary]) -> Option<u8> {
    slots.iter().find(|slot| !slot.used).map(|slot| slot.index)
}

pub fn merge_slot(
    vanilla: &[u8],
    coop: Option<Vec<u8>>,
    from_slot: u8,
    to_slot: u8,
) -> Result<Vec<u8>, SlotError> {
    if vanilla.len() != GAMEDATA_SIZE {
        return Err(SlotError::WrongSize(vanilla.len()));
    }
    let mut merged = match coop {
        Some(bytes) if bytes.len() == GAMEDATA_SIZE => bytes,
        Some(bytes) => return Err(SlotError::WrongSize(bytes.len())),
        None => blank_file_from(vanilla),
    };
    let source = slot_range(from_slot);
    let target = slot_range(to_slot);
    merged[target].copy_from_slice(&vanilla[source]);
    merged[HEADER_SLOT_MASK_OFFSET] |= 1 << to_slot;
    fix_header_checksum(&mut merged).map_err(|_| SlotError::WrongSize(merged.len()))?;
    Ok(merged)
}

pub fn copy_slot_into_coop(
    save_dir: &Path,
    from_slot: u8,
    to_slot: u8,
) -> Result<PathBuf, SlotError> {
    let not_found = |what: &str| std::io::Error::new(std::io::ErrorKind::NotFound, what.to_owned());
    let vanilla_path = gamedata_path(save_dir).ok_or_else(|| not_found("no normal save file"))?;
    let coop_path = coop_gamedata_path(save_dir).ok_or_else(|| not_found("no save folder"))?;
    let coop = match std::fs::read(&coop_path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let merged = merge_slot(&std::fs::read(vanilla_path)?, coop, from_slot, to_slot)?;
    drop_campaign(&coop_path, to_slot)?;
    let staging = coop_path.with_extension("coop.tmp");
    std::fs::write(&staging, merged)?;
    std::fs::rename(&staging, &coop_path)?;
    Ok(coop_path)
}

pub fn backup_coop_file(
    save_dir: &Path,
    coop_backup_root: &Path,
    stamp: &str,
) -> Result<PathBuf, BackupError> {
    let source = coop_gamedata_path(save_dir)
        .filter(|path| path.is_file())
        .ok_or_else(|| BackupError::NoSaveFolder(save_dir.to_path_buf()))?;
    let destination = coop_backup_root.join(stamp);
    if destination.exists() {
        return Err(BackupError::AlreadyExists(stamp.to_owned()));
    }
    std::fs::create_dir_all(&destination)?;
    std::fs::copy(&source, destination.join(COOP_GAMEDATA_FILE_NAME))?;
    copy_companions_into(&source, &destination)?;
    Ok(destination)
}

pub fn restore_coop_backup(
    save_dir: &Path,
    coop_backup_root: &Path,
    stamp: &str,
    now_stamp: &str,
) -> Result<PathBuf, BackupError> {
    let source = coop_backup_root.join(stamp).join(COOP_GAMEDATA_FILE_NAME);
    if !source.is_file() {
        return Err(BackupError::NoSaveFolder(source));
    }
    let bytes = std::fs::read(&source)?;
    if bytes.len() != GAMEDATA_SIZE {
        return Err(BackupError::WrongSize(bytes.len()));
    }
    let target = coop_gamedata_path(save_dir)
        .ok_or_else(|| BackupError::NoSaveFolder(save_dir.to_path_buf()))?;
    let safety_stamp = format!("{COOP_BEFORE_RESTORE_PREFIX}{now_stamp}");
    let safety_copy = if target.is_file() {
        Some(backup_coop_file(save_dir, coop_backup_root, &safety_stamp)?)
    } else {
        None
    };
    let staging = target.with_extension("coop.tmp");
    std::fs::write(&staging, bytes)?;
    std::fs::rename(&staging, &target)?;
    restore_companions_from(&coop_backup_root.join(stamp), &target)?;
    Ok(safety_copy.unwrap_or(source))
}

pub(super) fn slot_range(slot: u8) -> std::ops::Range<usize> {
    let start = SLOTS_START + usize::from(slot) * SLOT_STRIDE;
    start..start + SLOT_STRIDE
}

pub(super) fn blank_file_from(vanilla: &[u8]) -> Vec<u8> {
    let unused_template = parse_slots(vanilla).ok().and_then(|slots| {
        slots
            .iter()
            .find(|slot| !slot.used)
            .map(|slot| slot_range(slot.index))
    });
    let mut blank = vanilla.to_vec();
    for index in 0..SLOT_COUNT {
        let range = slot_range(index);
        match &unused_template {
            Some(template) => blank.copy_within(template.clone(), range.start),
            None => blank[range].fill(0),
        }
    }
    blank
}
