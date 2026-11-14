mod boss_bar_probe;
mod boss_hit_log;
mod boss_motion_probe;
mod bullet_hit_hook;
mod ca_gauge_probe;
mod combat;
mod combat_view;
mod companion;
mod companion_probe;
mod counters;
mod drop_hook;
mod game_calls;
mod gauge_opening_replay;
mod guest_combat;
mod hit_hook;
mod hold_hook;
mod host_combat;
mod install;
mod shared_actors;
mod spawn_hook;
mod status;
mod sync;
mod word_drop_hook;

pub(crate) use combat_view::applying_remote;
pub use counters::CombatCounts;
pub(crate) use gauge_opening_replay::{OpeningReplayStep, replay_guest_opening_hit};
pub(crate) use hold_hook::cut_copy_step_before_move;
pub use install::install;
pub use shared_actors::shared_enemy_actors;
pub use status::{
    EnemySyncRole, EnemySyncStatus, enemy_id_text, enemy_sync_status, set_snap_enabled,
    snap_enabled,
};
pub use sync::tick_enemy_sync;

const MAX_RECENT_ENEMY_EVENTS: usize = 6;
const COPY_KILL_ATTEMPTS: u32 = 3;
const MOTION_FRAME_MOVED: f32 = 0.01;
const MOTION_STALL_MS: u64 = 1000;
const MOTION_RESTART_INTERVAL_MS: u64 = 500;
const SECOND_MOTION_LAYER: u32 = 1;
const PENDING_KILL_MAX_AGE_MS: u64 = 3000;
