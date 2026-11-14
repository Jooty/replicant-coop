mod end_follow;
mod end_queue;
mod gate;
mod names;
mod partner_copies;
mod ran_ends;
mod relayed_scenes;
mod scene_pull;

pub use end_follow::{
    EndArrival, EndDrop, EndFollow, MaskIdleTimer, PendingEnd, WaitStep, control_slots_free,
    end_follow, is_end_callback, wait_step,
};
pub use end_queue::{EndQueue, ParkedCall, ParkedKind, QueueEvent, Released};
pub use gate::{CallbackGate, GateRole, GateStep, LocalFire, PartnerFire};
pub use names::{is_event_talk, is_gated_callback, is_scene_talk};
pub use partner_copies::{PartnerStartedCopies, event_base, event_family};
pub use ran_ends::RanEnds;
pub use relayed_scenes::{PlayedStart, RelayedScenesPlayed, scene_base, start_base};
pub use scene_pull::{ScenePull, ScenePullFacts, ScenePullSkip, scene_pull};

pub const GATED_CALLBACK_NAMES: [&str; 1] = ["Boss_Battle_Start"];
pub const GATED_EVENT_SUFFIX: &str = "_Start";
pub const SCENE_PLAY_SUFFIXES: [&str; 3] = ["_PLAY_EVENT", "_EVENT", "_Event"];
pub const STORY_EVENT_PREFIXES: [&str; 2] = ["EID_", "SEID_"];
pub const CALLBACK_GATE_PAIRING_WINDOW_MS: u64 = 20_000;
pub const CALLBACK_GATE_HOST_GRACE_MS: u64 = 3_000;
pub const CALLBACK_GATE_AFTER_DIALOG_CLOSE_MS: u64 = 600;
pub const CALLBACK_GATE_GUEST_GIVE_UP_MS: u64 = 20_000;
pub const SCENE_TALK_PREFIX: &str = "SEID_";
pub const END_FOLLOW_EVENT_SUFFIX: &str = "_End";
pub const END_FOLLOW_STILL_WAITING_LOG_MS: u64 = 60_000;
pub const END_FOLLOW_MASK_IDLE_MS: u64 = 500;
pub const TALK_EVENT_SUFFIX: &str = "_Talk";
pub const TALK_START_AFTER_COMMAND_MS: u64 = 2_000;
