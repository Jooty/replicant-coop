mod easing;
mod motion_clock;
mod sweep;
mod transition;

pub use easing::Easing;
pub use motion_clock::{MotionClock, drift_offset_px};
pub use sweep::Sweep;
pub use transition::Transition;

pub const SELECTION_SWEEP_MS: f32 = 120.0;
pub const SECTION_ENTER_MS: f32 = 180.0;
pub const SECTION_STAGGER_MS: f32 = 40.0;
pub const SEAT_ENTER_MS: f32 = 200.0;
pub const SEAT_STAGGER_MS: f32 = 90.0;
pub const SEAT_FIRST_DELAY_MS: f32 = 80.0;
pub const ART_FADE_MS: f32 = 220.0;
pub const STATUS_INVERT_MS: f32 = 140.0;
pub const ROW_ART_BRIGHTEN_MS: f32 = 160.0;
pub const HATCH_DRIFT_MS_PER_PX: f32 = 250.0;
pub const METER_FOLLOW_MS: f32 = 160.0;
