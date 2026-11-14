use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use nc_launcher_core::GameProfile;
use replicant_game::build::PE_HEADER_READ_LEN;

use crate::steam::library::{library_paths, manifest_install_dir};

const LIBRARY_FOLDERS_FILE: &str = "libraryfolders.vdf";
const REG_VALUE_TYPE: &str = "REG_SZ";
const WINDOWS_DEFAULT_STEAM_PATHS: [&str; 2] =
    ["C:\\Program Files (x86)\\Steam", "C:\\Program Files\\Steam"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameInstall {
    pub profile: &'static GameProfile,
    pub game_dir: PathBuf,
    pub steamapps_dir: PathBuf,
    pub proton_prefix: Option<PathBuf>,
}

impl GameInstall {
    pub fn exe_path(&self) -> PathBuf {
        let names = self.profile.exe_names;
        let present = names
            .iter()
            .map(|name| self.game_dir.join(name))
            .find(|path| path.is_file());
        present.unwrap_or_else(|| self.game_dir.join(names[0]))
    }

    pub fn exe_present(&self) -> bool {
        self.profile
            .exe_names
            .iter()
            .any(|name| self.game_dir.join(name).is_file())
    }

    /// Whether the game's exe is a build the mod supports, read from the head of the file.
    pub fn exe_build_supported(&self) -> bool {
        let mut header = Vec::with_capacity(PE_HEADER_READ_LEN);
        let read = File::open(self.exe_path()).and_then(|file| {
            file.take(PE_HEADER_READ_LEN as u64)
                .read_to_end(&mut header)
        });
        read.is_ok() && self.profile.supports_exe_header(&header)
    }

    pub fn proton_compat_dir(&self) -> PathBuf {
        self.steamapps_dir
            .join("compatdata")
            .join(self.profile.steam_app_id)
    }
}

pub fn candidate_steam_roots(
    home: Option<&Path>,
    registry_steam_path: Option<&str>,
) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(path) = registry_steam_path {
        roots.push(PathBuf::from(path));
    }
    if cfg!(windows) {
        roots.extend(WINDOWS_DEFAULT_STEAM_PATHS.iter().map(PathBuf::from));
    }
    if let Some(home) = home {
        roots.push(home.join(".steam/steam"));
        roots.push(home.join(".local/share/Steam"));
        roots.push(home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
        roots.push(home.join(".var/app/com.valvesoftware.Steam/.steam/steam"));
        roots.push(home.join("snap/steam/common/.local/share/Steam"));
        roots.push(home.join("snap/steam/common/.steam/steam"));
    }
    roots
}

pub fn parse_reg_query_value(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (_, rest) = line.split_once(REG_VALUE_TYPE)?;
        let value = rest.trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}

pub fn find_game(profile: &'static GameProfile, steam_roots: &[PathBuf]) -> Option<GameInstall> {
    let mut libraries: Vec<PathBuf> = Vec::new();
    for root in steam_roots {
        push_unique(&mut libraries, root.clone());
        let listing = [
            root.join("steamapps").join(LIBRARY_FOLDERS_FILE),
            root.join("config").join(LIBRARY_FOLDERS_FILE),
        ]
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok());
        for library in listing.iter().flat_map(|text| library_paths(text)) {
            push_unique(&mut libraries, library);
        }
    }
    libraries
        .iter()
        .find_map(|library| install_in_library(profile, library))
}

pub fn install_from_game_dir(profile: &'static GameProfile, game_dir: &Path) -> GameInstall {
    let steamapps_dir = game_dir
        .parent()
        .and_then(Path::parent)
        .map_or_else(|| game_dir.to_path_buf(), Path::to_path_buf);
    GameInstall {
        profile,
        proton_prefix: existing_prefix(profile, &steamapps_dir),
        game_dir: game_dir.to_path_buf(),
        steamapps_dir,
    }
}

fn install_in_library(profile: &'static GameProfile, library: &Path) -> Option<GameInstall> {
    let steamapps_dir = library.join("steamapps");
    let manifest = std::fs::read_to_string(
        steamapps_dir.join(format!("appmanifest_{}.acf", profile.steam_app_id)),
    )
    .ok()?;
    let install_dir = manifest_install_dir(&manifest)?;
    let game_dir = steamapps_dir.join("common").join(install_dir);
    game_dir.is_dir().then(|| GameInstall {
        profile,
        proton_prefix: existing_prefix(profile, &steamapps_dir),
        game_dir,
        steamapps_dir,
    })
}

fn existing_prefix(profile: &GameProfile, steamapps_dir: &Path) -> Option<PathBuf> {
    let prefix = steamapps_dir
        .join("compatdata")
        .join(profile.steam_app_id)
        .join("pfx");
    prefix.is_dir().then_some(prefix)
}

fn push_unique(list: &mut Vec<PathBuf>, path: PathBuf) {
    if !list.contains(&path) {
        list.push(path);
    }
}
