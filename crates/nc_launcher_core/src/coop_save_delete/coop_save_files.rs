use std::path::{Path, PathBuf};

use crate::coop_save_delete::COOP_SAVE_NAME_PREFIXES;

/// Whether a name in a save folder is one of the mod's co-op saves. The game's own save never is.
pub fn is_coop_save_name(name: &str) -> bool {
    COOP_SAVE_NAME_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// Every co-op save file and folder directly inside the given save folders, sorted. A folder that cannot
/// be read holds none.
pub fn coop_saves_in(save_folders: &[PathBuf]) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = save_folders
        .iter()
        .flat_map(|folder| entries_named_as_coop_saves(folder))
        .collect();
    found.sort();
    found
}

fn entries_named_as_coop_saves(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().into_string().ok()?;
            is_coop_save_name(&name).then(|| entry.path())
        })
        .collect()
}
