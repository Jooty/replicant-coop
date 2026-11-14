pub mod app;
pub mod config;
pub mod game_link;
pub mod home;
pub mod install;
pub mod lobby;
pub mod logs;
pub mod net;
pub mod proton;
pub mod saves;
pub mod seat_round;
pub mod second_copy;
pub mod steam;
pub mod update;

pub const DEV_BUILD: bool = cfg!(feature = "dev");
