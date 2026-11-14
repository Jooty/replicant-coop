mod command;
mod launch;
mod runtime;

pub use command::{
    CopyCommand, ProtonSetup, copy_command, copy_environment, proton_script_from_config_info,
};
pub use launch::{CopyExit, CopyOutput, SecondCopy, build_command, start_second_copy};
pub use runtime::{required_tool_app_id, runtime_entry_point, steam_client_root};

pub const LINK_ENV: &str = "NIER_COOP_LINK";
pub const NAME_ENV: &str = "NIER_COOP_NAME";
pub const BOOT_SLOT_ENV: &str = "NIER_COOP_BOOT_SLOT";
pub const LOG_ENV: &str = "NIER_COOP_LOG";
pub const NEW_GAME_SLOT_ENV: &str = "NIER_COOP_NEW_GAME_SLOT";
pub const NEW_GAME_NAME_ENV: &str = "NIER_COOP_NEW_GAME_NAME";
pub const BLOCK_SAVES_ENV: &str = "NIER_COOP_BLOCK_SAVES";
pub const PROTON_SCRIPT_NAME: &str = "proton";
pub const TOOL_MANIFEST_NAME: &str = "toolmanifest.vdf";
pub const RUNTIME_ENTRY_POINT_NAME: &str = "_v2-entry-point";
pub const STEAM_LAUNCHER_SCRIPT_NAME: &str = "steam.sh";
pub const SECOND_COPY_LOG_NAME: &str = "second-copy.log";
pub const LOGS_FOLDER_NAME: &str = "logs";
pub const EARLY_EXIT_SECONDS: f64 = 20.0;
