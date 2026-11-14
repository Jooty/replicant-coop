mod host_led_runs;
mod report;
mod script_pause;
mod slot_holder;

pub use host_led_runs::HostLedRuns;
pub use report::{
    PauseCall, ScriptRunnerLogGate, ScriptRunnerSnapshot, format_empty_pop, format_pause_call,
    format_runner_state, format_slot_holder,
};
pub use script_pause::ScriptPauseWatch;
pub use slot_holder::{SlotHolderReport, SlotHolderWatch};

pub const SCRIPT_PAUSE_STALE_AFTER_MS: u64 = 30_000;
pub const PAUSE_ON_NATIVE_RVA: u32 = 0x6E_E710;
pub const PAUSE_OFF_NATIVE_RVA: u32 = 0x6E_E700;
pub const QUEUE_FIRST_SLOT_OFFSET: u64 = 0x8_0BE8;
pub const QUEUE_SLOT_STRIDE: u64 = 0x7A8;
pub const QUEUE_SLOT_COUNT: u64 = 64;
pub const RUNNER_LOG_INTERVAL_MS: u64 = 1_000;
pub const SLOT_HOLDER_LOGGED_EVERY_MS: u64 = 5_000;
pub const SLOT_HOLDER_SKIPPED_PER_FRAME_NATIVES: [&str; 1] = ["_DebugPrint"];
