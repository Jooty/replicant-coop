use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDateTime, TimeZone};
use thiserror::Error;

const STAMP_FORMAT: &str = "%Y%m%d-%H%M%S";
const STAMP_LABEL_FORMAT: &str = "%d %b, %H:%M";

#[derive(Debug, Error)]
pub enum BackupError {
    #[error("save folder not found: {0}")]
    NoSaveFolder(PathBuf),
    #[error("backup {0} already exists; refusing to overwrite")]
    AlreadyExists(String),
    #[error("backup has {0} bytes, expected the size of a save file")]
    WrongSize(usize),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

pub fn format_stamp<Tz: TimeZone>(moment: &DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    moment.format(STAMP_FORMAT).to_string()
}

/// A backup's stamp the way the backup list reads it ("12 Nov, 13:40"); a stamp of another shape is shown as it is.
pub fn stamp_label(stamp: &str) -> String {
    NaiveDateTime::parse_from_str(stamp, STAMP_FORMAT).map_or_else(
        |_| stamp.to_owned(),
        |moment| moment.format(STAMP_LABEL_FORMAT).to_string(),
    )
}

pub fn backup_saves(
    save_dir: &Path,
    backup_root: &Path,
    stamp: &str,
) -> Result<(PathBuf, usize), BackupError> {
    if !save_dir.is_dir() {
        return Err(BackupError::NoSaveFolder(save_dir.to_path_buf()));
    }
    let destination = backup_root.join(stamp);
    if destination.exists() {
        return Err(BackupError::AlreadyExists(stamp.to_owned()));
    }
    std::fs::create_dir_all(backup_root)?;
    let files = copy_dir_recursive(save_dir, &destination)?;
    Ok((destination, files))
}

pub fn copy_dir_recursive(from: &Path, to: &Path) -> std::io::Result<usize> {
    std::fs::create_dir_all(to)?;
    let mut files = 0;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            files += copy_dir_recursive(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
            files += 1;
        }
    }
    Ok(files)
}
