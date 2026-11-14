use std::ffi::CString;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::save_files::{
    COOP_GAMEDATA_BACKUP_SUFFIX, COOP_GAMEDATA_FILE_NAME, COOP_SAVE_NAME_MAX_BYTES,
    GAMEDATA_FILE_NAME,
};
use crate::story_progress::RANK_SIDECAR_SUFFIX;

/// The name of the file the game's own save is redirected to, beside the normal save. Only a
/// plain file name that starts with the normal save's name and a dot is accepted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoopSaveName {
    name: CString,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoopSaveNameRefusal {
    Empty,
    HasPathSeparator,
    NotBesideTheNormalSave,
    TooLong,
    UnsafeCharacter,
    NamesADerivedFile,
}

/// Every file the mod derives from the co-op file's name, in one save folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoopSavePaths {
    pub file: PathBuf,
    pub backup: PathBuf,
    pub rank_sidecar: PathBuf,
}

impl CoopSaveName {
    /// The name from the link file, or the default co-op file when the link file names none.
    pub fn parse(named: Option<&str>) -> Result<Self, CoopSaveNameRefusal> {
        let name = named.unwrap_or(COOP_GAMEDATA_FILE_NAME);
        if let Some(refusal) = refusal_of(name) {
            return Err(refusal);
        }
        CString::new(name)
            .map(|name| Self { name })
            .map_err(|_| CoopSaveNameRefusal::UnsafeCharacter)
    }

    pub fn file_name(&self) -> &str {
        self.name.to_str().unwrap_or(COOP_GAMEDATA_FILE_NAME)
    }

    pub fn with_nul(&self) -> &[u8] {
        self.name.as_bytes_with_nul()
    }

    pub fn paths_in(&self, folder: &Path) -> CoopSavePaths {
        let name = self.file_name();
        CoopSavePaths {
            file: folder.join(name),
            backup: folder.join(format!("{name}{COOP_GAMEDATA_BACKUP_SUFFIX}")),
            rank_sidecar: folder.join(format!("{name}{RANK_SIDECAR_SUFFIX}")),
        }
    }
}

impl Default for CoopSaveName {
    fn default() -> Self {
        Self {
            name: CString::new(COOP_GAMEDATA_FILE_NAME).unwrap_or_default(),
        }
    }
}

impl fmt::Display for CoopSaveNameRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "the name is empty",
            Self::HasPathSeparator => "the name holds a path separator",
            Self::NotBesideTheNormalSave => {
                "the name does not start with the normal save's name and a dot"
            }
            Self::TooLong => "the name is too long",
            Self::UnsafeCharacter => "the name holds a character other than letters, digits, . _ -",
            Self::NamesADerivedFile => "the name ends like the backup or the story sidecar",
        })
    }
}

fn refusal_of(name: &str) -> Option<CoopSaveNameRefusal> {
    if name.is_empty() {
        return Some(CoopSaveNameRefusal::Empty);
    }
    if name.contains(['/', '\\']) {
        return Some(CoopSaveNameRefusal::HasPathSeparator);
    }
    if name.len() > COOP_SAVE_NAME_MAX_BYTES {
        return Some(CoopSaveNameRefusal::TooLong);
    }
    let allowed = |character: char| character.is_ascii_alphanumeric() || "._-".contains(character);
    if !name.chars().all(allowed) {
        return Some(CoopSaveNameRefusal::UnsafeCharacter);
    }
    let after_prefix = name
        .strip_prefix(GAMEDATA_FILE_NAME)
        .and_then(|rest| rest.strip_prefix('.'));
    if after_prefix.is_none_or(str::is_empty) {
        return Some(CoopSaveNameRefusal::NotBesideTheNormalSave);
    }
    if [COOP_GAMEDATA_BACKUP_SUFFIX, RANK_SIDECAR_SUFFIX]
        .iter()
        .any(|ending| name.ends_with(ending))
    {
        return Some(CoopSaveNameRefusal::NamesADerivedFile);
    }
    None
}
