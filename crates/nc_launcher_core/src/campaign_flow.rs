mod guest_seat_sync;
mod guest_start_plan;
mod host_start_gate;
mod newest_seat;
mod refusal_words;
mod session_file_name;

pub const SEAT_DELIVERY_TIMEOUT_SECONDS: f64 = 60.0;
pub const SESSION_FILE_PREFIX: &str = "GAMEDATA.seat-";
pub const SESSION_FILE_ID_CHARACTERS: usize = 8;
pub const MIN_CAMPAIGN_ID_CHARACTERS: usize = 8;
pub const MAX_CAMPAIGN_ID_CHARACTERS: usize = 64;

pub use guest_seat_sync::GuestSeatSync;
pub use guest_start_plan::{GuestStartPlan, plan_guest_start};
pub use host_start_gate::{HostStartGate, StartGateStep};
pub use newest_seat::{
    HostStore, SeatCandidate, SeatSource, host_store_decision, pick_newest_seat,
};
pub use refusal_words::{seat_error_words, seat_refusal_words};
pub use session_file_name::{campaign_file_stem, session_file_name};
