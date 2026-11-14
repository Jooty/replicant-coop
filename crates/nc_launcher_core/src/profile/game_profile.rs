use replicant_game::build::PeIdentity;

use crate::profile::{CharacterArt, GameMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Character {
    pub id: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameSupport {
    pub install: bool,
    pub start: bool,
    pub saves: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameProfile {
    pub game_id: &'static str,
    pub display_name: &'static str,
    pub steam_app_id: &'static str,
    pub install_folder_name: &'static str,
    pub exe_names: &'static [&'static str],
    pub supported_exe_builds: &'static [PeIdentity],
    pub loader_dll_name: &'static str,
    pub hook_dll_name: &'static str,
    pub link_file_name: &'static str,
    pub characters: &'static [Character],
    pub characters_shareable: bool,
    pub character_art: &'static [CharacterArt],
    pub modes: &'static [GameMode],
    pub support: GameSupport,
}

const DLL_SUFFIX: &str = ".dll";

impl GameProfile {
    pub fn loader_dll_stem(&self) -> &'static str {
        self.loader_dll_name
            .strip_suffix(DLL_SUFFIX)
            .unwrap_or(self.loader_dll_name)
    }

    /// Whether an exe whose first bytes are `header` is a build of the game the mod supports.
    pub fn supports_exe_header(&self, header: &[u8]) -> bool {
        PeIdentity::check_supported(header, self.supported_exe_builds).is_ok()
    }

    pub fn is_game_exe(&self, file_name: &str) -> bool {
        self.exe_names.contains(&file_name)
    }

    pub fn supports_anything(&self) -> bool {
        self.support.install || self.support.start || self.support.saves
    }

    pub fn mode(&self, id: u8) -> Option<&'static GameMode> {
        self.modes.iter().find(|mode| mode.id == id)
    }

    pub fn character(&self, id: &str) -> Option<&'static Character> {
        self.characters.iter().find(|character| character.id == id)
    }

    pub fn character_art(&self, id: &str) -> Option<&'static CharacterArt> {
        self.character_art.iter().find(|art| art.character_id == id)
    }
}
