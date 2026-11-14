use std::path::{Path, PathBuf};

use crate::config::data_dir;
use crate::saves::{
    BACKUP_FOLDER, COOP_BACKUP_FOLDER, MY_GAMES_FOLDER, PROTON_SAVE_PREFIX_SEGMENTS,
    STEAM_SAVE_FOLDER,
};
use crate::steam::GameInstall;

const DOCUMENTS_FOLDER: &str = "Documents";

pub fn save_dir(game: &GameInstall) -> Option<PathBuf> {
    if !game.profile.support.saves {
        return None;
    }
    let documents = match &game.proton_prefix {
        Some(prefix) if !cfg!(windows) => PROTON_SAVE_PREFIX_SEGMENTS
            .iter()
            .fold(prefix.clone(), |path, segment| path.join(segment))
            .join(DOCUMENTS_FOLDER),
        _ => dirs::document_dir()?,
    };
    Some(
        documents
            .join(MY_GAMES_FOLDER)
            .join(game.profile.install_folder_name),
    )
}

pub fn backup_root() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join(BACKUP_FOLDER))
}

pub fn coop_backup_root() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join(COOP_BACKUP_FOLDER))
}

/// The folders the game keeps its saves in, one per Steam account under the save folder.
pub fn save_folders(save_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(save_dir.join(STEAM_SAVE_FOLDER)) else {
        return Vec::new();
    };
    let mut folders: Vec<PathBuf> = entries
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| path.is_dir())
        .collect();
    folders.sort();
    folders
}
