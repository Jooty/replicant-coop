mod cast_log;
mod kinds;
mod scan;
mod table;

pub use cast_log::{CastSummary, PowerCastLog};
pub use kinds::{POWER_MODEL_KINDS, power_model_is_prop, power_model_pack_name};
pub use scan::{SightedHand, hands_of_player};
pub use table::PowerModelTable;

pub const POWER_MODEL_SCAN_INTERVAL_MS: u64 = 50;
pub const POWER_MODEL_STALE_MS: u64 = 800;
pub const MAX_SIGHTED_HANDS: usize = 3;
pub const MAX_PARTNER_POWER_MODELS: usize = 6;
