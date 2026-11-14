//! Telling a stopped game thread apart from a crash: its heartbeat, the mod step it is in, and
//! when a watcher samples where it stands.

mod heartbeat;
mod schedule;
mod steps;
mod text;

pub use heartbeat::{
    GameThreadHeartbeat, game_thread_heartbeat, note_game_thread_tick, stall_clock_ms,
};
pub use schedule::{StallVerdict, StallWatch};
pub use steps::{GameThreadSteps, game_thread_steps, in_game_thread_step};
pub use text::{
    StallSample, game_thread_ended_line, stall_recovered_line, stall_sample_text,
    stall_unreadable_line,
};

pub const STALL_SAMPLE_AFTER_MS: [u64; 3] = [6_000, 12_000, 24_000];
pub const STALL_LATER_SAMPLE_EVERY_MS: u64 = 60_000;
pub const STALL_MAX_SAMPLES_PER_STALL: u32 = 6;
pub const STALL_WATCH_INTERVAL_MS: u64 = 500;
