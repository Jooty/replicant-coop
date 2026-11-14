mod actions;
mod console_facts;
mod console_run;
mod coop_save_menu;
mod ended_lobby;
mod events;
#[cfg(feature = "snapshots")]
mod fixture;
mod forms;
mod game_look;
mod join_dialog;
mod lobby_failure;
mod log_upload;
mod motion;
mod pages;
pub mod palette;
pub mod parts;
mod screen;
mod state;
mod style;
mod tasks;
mod updates;
mod window;

pub use actions::{ActionReport, run_install, run_uninstall};
pub use console_facts::console_script;
pub use console_run::ConsoleRun;
pub use coop_save_menu::CoopSaveMenu;
pub use ended_lobby::EndedLobby;
pub use events::{describe_event, event_log_line};
#[cfg(feature = "snapshots")]
pub use fixture::{draw_parts_sheet, fixture_campaign_card, fixture_launcher};
pub use forms::NetForms;
pub use game_look::{GameLook, GameMark, TitleFace, game_look};
pub use join_dialog::JoinDialog;
pub use lobby_failure::LobbyFailure;
pub use log_upload::LogUploader;
pub use pages::{Tab, draw};
pub use parts::copy_line_id;
pub use screen::Screen;
pub use state::{Launcher, Role};
pub use style::{apply_launcher_look, heading_text, launcher_visuals};
pub use tasks::Task;
pub use updates::{
    RESTART_TAB_ARGUMENT, RESTART_TAB_LOGS, RESTART_TAB_SETTINGS, RESTART_UPDATED_ARGUMENT,
    UpdateState, Updater,
};
pub use window::run;

pub const WINDOW_TITLE: &str = "Nier Together";
pub const WINDOW_WIDTH: f32 = 1180.0;
pub const WINDOW_HEIGHT: f32 = 760.0;
pub const WINDOW_MIN_WIDTH: f32 = 900.0;
pub const WINDOW_MIN_HEIGHT: f32 = 600.0;
pub const STEAM_RUN_URL_PREFIX: &str = "steam://rungameid/";
pub const GAME_POLL_SECONDS: f64 = 2.0;
