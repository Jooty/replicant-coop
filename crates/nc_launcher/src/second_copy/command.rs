use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::proton::find_proton_files_dir;
use crate::second_copy::{
    BLOCK_SAVES_ENV, BOOT_SLOT_ENV, LINK_ENV, NAME_ENV, NEW_GAME_NAME_ENV, NEW_GAME_SLOT_ENV,
    PROTON_SCRIPT_NAME,
};
use crate::steam::GameInstall;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyCommand {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub envs: Vec<(String, String)>,
    pub current_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtonSetup {
    pub script: PathBuf,
    pub steam_root: PathBuf,
    pub runtime_entry: Option<PathBuf>,
}

impl CopyCommand {
    pub fn shell_line(&self) -> String {
        let mut parts = vec![
            format!("cd {}", quote(&self.current_dir.to_string_lossy())),
            "&&".to_owned(),
            "env".to_owned(),
        ];
        parts.extend(
            self.envs
                .iter()
                .map(|(key, value)| format!("{key}={}", quote(value))),
        );
        parts.push(quote(&self.program.to_string_lossy()));
        parts.extend(self.args.iter().map(|arg| quote(&arg.to_string_lossy())));
        parts.join(" ")
    }

    pub fn env_names(&self) -> String {
        self.envs
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

pub fn copy_environment(
    link_port: u16,
    name: &str,
    boot_slot: Option<u8>,
    new_game: Option<(u8, &str)>,
) -> Vec<(String, String)> {
    let mut envs = vec![
        (LINK_ENV.to_owned(), link_port.to_string()),
        (NAME_ENV.to_owned(), name.to_owned()),
        (BLOCK_SAVES_ENV.to_owned(), "1".to_owned()),
    ];
    if let Some(slot) = boot_slot {
        envs.push((BOOT_SLOT_ENV.to_owned(), slot.to_string()));
    }
    if let Some((slot, game_name)) = new_game {
        envs.push((NEW_GAME_SLOT_ENV.to_owned(), slot.to_string()));
        envs.push((NEW_GAME_NAME_ENV.to_owned(), game_name.to_owned()));
    }
    envs
}

pub fn proton_script_from_config_info(config_info: &str) -> Option<PathBuf> {
    let files_dir = find_proton_files_dir(config_info)?;
    Some(files_dir.parent()?.join(PROTON_SCRIPT_NAME))
}

pub fn copy_command(
    game: &GameInstall,
    copy_envs: Vec<(String, String)>,
    proton: Option<&ProtonSetup>,
) -> CopyCommand {
    let exe = game.exe_path();
    let mut envs = copy_envs;
    let Some(proton) = proton else {
        return CopyCommand {
            program: exe,
            args: Vec::new(),
            envs,
            current_dir: game.game_dir.clone(),
        };
    };
    let library = &game.steamapps_dir;
    let tool_dir = proton.script.parent().unwrap_or(Path::new("/"));
    let mut tool_paths = tool_dir.display().to_string();
    if let Some(runtime_dir) = proton
        .runtime_entry
        .as_ref()
        .and_then(|entry| entry.parent())
    {
        tool_paths.push(':');
        tool_paths.push_str(&runtime_dir.display().to_string());
    }
    envs.extend([
        (
            "STEAM_COMPAT_DATA_PATH".to_owned(),
            game.proton_compat_dir().display().to_string(),
        ),
        (
            "STEAM_COMPAT_CLIENT_INSTALL_PATH".to_owned(),
            proton.steam_root.display().to_string(),
        ),
        (
            "STEAM_COMPAT_INSTALL_PATH".to_owned(),
            game.game_dir.display().to_string(),
        ),
        (
            "STEAM_COMPAT_LIBRARY_PATHS".to_owned(),
            library.display().to_string(),
        ),
        ("STEAM_COMPAT_TOOL_PATHS".to_owned(), tool_paths),
        (
            "STEAM_COMPAT_APP_ID".to_owned(),
            game.profile.steam_app_id.to_owned(),
        ),
        (
            "SteamAppId".to_owned(),
            game.profile.steam_app_id.to_owned(),
        ),
        (
            "SteamGameId".to_owned(),
            game.profile.steam_app_id.to_owned(),
        ),
    ]);
    match &proton.runtime_entry {
        Some(entry) => CopyCommand {
            program: entry.clone(),
            args: vec![
                OsString::from("--verb=run"),
                OsString::from("--"),
                proton.script.clone().into_os_string(),
                OsString::from("run"),
                exe.into_os_string(),
            ],
            envs,
            current_dir: game.game_dir.clone(),
        },
        None => CopyCommand {
            program: proton.script.clone(),
            args: vec![OsString::from("run"), exe.into_os_string()],
            envs,
            current_dir: game.game_dir.clone(),
        },
    }
}

fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}
