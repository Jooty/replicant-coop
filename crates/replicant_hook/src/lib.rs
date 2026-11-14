#[cfg(windows)]
mod actions;
#[cfg(windows)]
mod attack_gauge;
#[cfg(windows)]
mod blocks;
#[cfg(windows)]
mod bombs;
#[cfg(windows)]
mod boot;
#[cfg(windows)]
mod boss_messages;
#[cfg(windows)]
mod bullets;
#[cfg(windows)]
mod carts;
#[cfg(windows)]
mod containers;
#[cfg(windows)]
mod cutscene_skip;
#[cfg(windows)]
mod desync;
#[cfg(windows)]
mod dialogue;
#[cfg(windows)]
mod door_sync;
#[cfg(windows)]
mod downed;
#[cfg(windows)]
mod effects;
#[cfg(windows)]
mod enemies;
#[cfg(windows)]
mod enemy_bullets;
#[cfg(windows)]
mod entry;
pub mod event_log;
#[cfg(windows)]
mod fade_sync;
#[cfg(windows)]
mod fight_sync;
#[cfg(windows)]
mod frame_tuning;
#[cfg(windows)]
mod game_hooks;
#[cfg(windows)]
mod gimmick_probe;
#[cfg(windows)]
mod kaine;
#[cfg(windows)]
mod lift_rides;
#[cfg(windows)]
mod minimap;
#[cfg(windows)]
mod move_log;
#[cfg(windows)]
mod music;
pub mod net;
#[cfg(windows)]
mod npcs;
#[cfg(windows)]
mod one_sided;
#[cfg(windows)]
mod overlay;
#[cfg(windows)]
mod paused_hits;
#[cfg(windows)]
mod pickups;
#[cfg(windows)]
mod player_body;
#[cfg(windows)]
mod player_names;
#[cfg(windows)]
mod player_settings;
#[cfg(windows)]
mod prologue_levels;
#[cfg(windows)]
mod proxy;
#[cfg(windows)]
mod puppet;
#[cfg(windows)]
mod render_camera;
#[cfg(windows)]
mod rewind;
#[cfg(windows)]
mod saves;
#[cfg(windows)]
mod script_rolls;
#[cfg(windows)]
mod sky;
#[cfg(windows)]
mod stay_together;
#[cfg(windows)]
mod story_jump;
#[cfg(windows)]
mod story_progress;
#[cfg(windows)]
mod story_relay;
#[cfg(windows)]
mod story_writes;
#[cfg(windows)]
mod teleport;
#[cfg(windows)]
mod zone_rules;

#[cfg(windows)]
use nc_hook::{build_gate, diagnostics, memory};

#[cfg(windows)]
pub use nc_hook::build_gate::{GateError, VerifiedBuild};
#[cfg(windows)]
pub use nc_hook::detour::{Detour, DetourError};
#[cfg(windows)]
pub use nc_hook::memory::InProcessMemory;
