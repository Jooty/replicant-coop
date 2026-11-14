use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::coop_save_delete::{DELETED_SAVES_FOLDER_SUFFIX, DELETED_SAVES_FOLDERS_KEPT};

/// Where a delete put the co-op saves, and how many files it moved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletedCoopSaves {
    pub folder: PathBuf,
    pub files: usize,
}

#[derive(Debug, Error)]
pub enum CoopSaveDeleteError {
    #[error("there are no co-op saves to delete")]
    NothingToDelete,
    #[error("{0} already exists; not writing over it")]
    AlreadyExists(PathBuf),
    #[error("could not copy the co-op saves aside, so none were deleted: {0}")]
    CopyFailed(io::Error),
    #[error("the copy of {0} does not match it, so no co-op saves were deleted")]
    CopyDiffers(PathBuf),
    #[error("could not delete the co-op saves, so they were put back: {0}")]
    RemoveFailed(io::Error),
    #[error(
        "could not delete the co-op saves or put them all back; every one is in {folder}: {error}"
    )]
    PutBackFailed { folder: PathBuf, error: io::Error },
}

struct PlacedSave {
    original: PathBuf,
    copy: PathBuf,
}

/// Moves a game's co-op saves into `<backups_root>/<stamp>-deleted/`, each at its place under `save_dir`:
/// copies them all, checks every copy, then removes the originals. A failure at any step leaves the save
/// folder as it was. Only the newest few such folders are kept.
pub fn delete_coop_saves(
    save_dir: &Path,
    coop_saves: &[PathBuf],
    backups_root: &Path,
    stamp: &str,
) -> Result<DeletedCoopSaves, CoopSaveDeleteError> {
    if coop_saves.is_empty() {
        return Err(CoopSaveDeleteError::NothingToDelete);
    }
    let folder = backups_root.join(format!("{stamp}{DELETED_SAVES_FOLDER_SUFFIX}"));
    if folder.exists() {
        return Err(CoopSaveDeleteError::AlreadyExists(folder));
    }
    let placed: Vec<PlacedSave> = coop_saves
        .iter()
        .map(|original| PlacedSave {
            original: original.clone(),
            copy: folder.join(place_under(save_dir, original)),
        })
        .collect();
    let files = match copy_every_save(&placed) {
        Ok(files) => files,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&folder);
            return Err(CoopSaveDeleteError::CopyFailed(error));
        }
    };
    if let Some(differing) = placed
        .iter()
        .find(|save| !same_contents(&save.original, &save.copy))
    {
        let _ = std::fs::remove_dir_all(&folder);
        return Err(CoopSaveDeleteError::CopyDiffers(differing.original.clone()));
    }
    if let Err(error) = remove_originals(&placed) {
        return Err(match put_back(&placed) {
            Ok(()) => {
                let _ = std::fs::remove_dir_all(&folder);
                CoopSaveDeleteError::RemoveFailed(error)
            }
            Err(put_back_error) => CoopSaveDeleteError::PutBackFailed {
                folder,
                error: put_back_error,
            },
        });
    }
    keep_newest_deleted_folders(backups_root, DELETED_SAVES_FOLDERS_KEPT);
    Ok(DeletedCoopSaves { folder, files })
}

/// Removes all but the `kept` newest `-deleted` folders in `backups_root`; returns how many it removed.
/// Every other folder there is left alone.
pub fn keep_newest_deleted_folders(backups_root: &Path, kept: usize) -> usize {
    let Ok(entries) = std::fs::read_dir(backups_root) else {
        return 0;
    };
    let mut deleted_folders: Vec<PathBuf> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().into_string().ok()?;
            (entry.file_type().ok()?.is_dir() && name.ends_with(DELETED_SAVES_FOLDER_SUFFIX))
                .then(|| entry.path())
        })
        .collect();
    deleted_folders.sort();
    let surplus = deleted_folders.len().saturating_sub(kept);
    deleted_folders
        .iter()
        .take(surplus)
        .filter(|folder| std::fs::remove_dir_all(folder).is_ok())
        .count()
}

fn place_under(save_dir: &Path, original: &Path) -> PathBuf {
    original.strip_prefix(save_dir).map_or_else(
        |_| original.file_name().map(PathBuf::from).unwrap_or_default(),
        Path::to_path_buf,
    )
}

fn copy_every_save(placed: &[PlacedSave]) -> io::Result<usize> {
    placed.iter().try_fold(0, |files, save| {
        Ok(files + copy_entry(&save.original, &save.copy)?)
    })
}

fn copy_entry(from: &Path, to: &Path) -> io::Result<usize> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if !std::fs::symlink_metadata(from)?.is_dir() {
        std::fs::copy(from, to)?;
        return Ok(1);
    }
    std::fs::create_dir_all(to)?;
    let mut files = 0;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        files += copy_entry(&entry.path(), &to.join(entry.file_name()))?;
    }
    Ok(files)
}

fn same_contents(original: &Path, copy: &Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(original) else {
        return false;
    };
    if !metadata.is_dir() {
        return matches!(
            (std::fs::read(original), std::fs::read(copy)),
            (Ok(left), Ok(right)) if left == right
        );
    }
    let (Ok(originals), Ok(copies)) = (std::fs::read_dir(original), std::fs::read_dir(copy)) else {
        return false;
    };
    let originals: Vec<_> = originals.filter_map(Result::ok).collect();
    originals.len() == copies.count()
        && originals
            .iter()
            .all(|entry| same_contents(&entry.path(), &copy.join(entry.file_name())))
}

fn remove_originals(placed: &[PlacedSave]) -> io::Result<()> {
    placed
        .iter()
        .try_for_each(|save| remove_entry(&save.original))
}

fn remove_entry(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => std::fs::remove_dir_all(path),
        Ok(_) => std::fs::remove_file(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn put_back(placed: &[PlacedSave]) -> io::Result<()> {
    for save in placed {
        if same_contents(&save.copy, &save.original) {
            continue;
        }
        // A folder that was only partly removed goes back whole.
        remove_entry(&save.original)?;
        copy_entry(&save.copy, &save.original)?;
    }
    Ok(())
}
