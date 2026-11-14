use std::path::PathBuf;
use std::process::Command;

use nc_launcher_core::GameProfile;

use crate::config::LauncherConfig;
use crate::steam::{
    GameInstall, candidate_steam_roots, find_game, install_from_game_dir, parse_reg_query_value,
};

const REGISTRY_KEY: &str = "HKCU\\Software\\Valve\\Steam";
const REGISTRY_VALUE: &str = "SteamPath";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn detect_game(profile: &'static GameProfile, config: &LauncherConfig) -> Option<GameInstall> {
    if let Some(game_dir) = config.game_dir_for(profile).filter(|dir| dir.is_dir()) {
        return Some(install_from_game_dir(profile, game_dir));
    }
    find_game(profile, &steam_roots())
}

/// Every place a Steam install may live on this PC: the registry's path on Windows, then the usual folders.
pub fn steam_roots() -> Vec<PathBuf> {
    let registry_path = read_registry_steam_path();
    candidate_steam_roots(dirs::home_dir().as_deref(), registry_path.as_deref())
}

pub fn read_registry_steam_path() -> Option<String> {
    if !cfg!(windows) {
        return None;
    }
    let mut command = Command::new("reg");
    command.args(["query", REGISTRY_KEY, "/v", REGISTRY_VALUE]);
    hide_console_window(&mut command);
    let output = command.output().ok()?;
    parse_reg_query_value(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(windows)]
pub fn hide_console_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub fn hide_console_window(_command: &mut Command) {
    let _ = CREATE_NO_WINDOW;
}
