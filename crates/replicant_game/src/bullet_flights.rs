mod scan;
mod shot_cue_board;
mod smoothing;
mod table;
mod tracker;

pub use scan::{SightedBullet, bullets_of_player, partner_effect_id, read_bullet, style_is_set};
pub use shot_cue_board::{
    CueOnCopy, NO_COPY_TO_END, NO_COPY_TO_FOLLOW, ShotCueBoard, one_shot_transform,
};
pub use smoothing::{FlightSample, smoothed_transform};
pub use table::BulletFlightTable;
pub use tracker::BulletTracker;

pub const BULLET_SCAN_INTERVAL_MS: u64 = 40;
pub const BULLET_FLIGHT_STALE_MS: u64 = 600;
pub const BULLET_LOOK_REPEAT_EVERY_MESSAGES: u32 = 10;
pub const MAX_SIGHTED_BULLETS: usize = 8;
pub const MAX_PARTNER_BULLET_FLIGHTS: usize = 8;
pub const BULLET_RENDER_DELAY_MS: u64 = 60;
pub const MAX_EXTRAPOLATION_MS: u64 = 250;
pub const BULLET_STATE_RUNNING: i32 = 0;
