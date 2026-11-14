//! Host enemies' bullets: sent by the host, drawn on the guest as effects or through the game's
//! sprite batch (visual only).

mod script_commands;
mod send;
mod shot_cues;
mod shot_sounds;
mod show;
mod sprite_draw;
mod tick;

pub use script_commands::install as install_script_commands;
pub(crate) use shot_cues::{note_effect_made, note_sound_made};
pub use sprite_draw::install;
pub use tick::tick;

const ENEMY_BULLET_SUMMARY_INTERVAL_MS: u64 = 5000;
