mod announce;
mod follow;
mod go_to_partner;
mod host_view;
mod leash;
mod phase_reads;
mod placement;
mod scene_end;
mod script_jump;
mod spot;
mod teleport_check;

pub use announce::{AnnounceConditions, HostAnnouncer};
pub use follow::{FollowConditions, FollowRole, FollowState, FollowStep, PhaseTarget, WaitReason};
pub use go_to_partner::{GoToPartnerRefusal, check_partner_reachable};
pub use host_view::{HostViewFacts, HostViewReading, HostViewTracker};
pub use leash::{Leash, distance_between};
pub use phase_reads::{announced_target, local_phase_index};
pub use placement::{PlacementCheck, PlacementVerdict, ReadBack, decide_placement};
pub use scene_end::{SceneEndFacts, SceneEndOwed, SceneEndWatch};
pub use script_jump::{JumpFacts, JumpPartner, JumpStep, ScriptJumpWatch};
pub use spot::{FloorHit, SpotRefusal, WorldProbe, choose_spot};
pub use teleport_check::{
    TELEPORT_ARRIVED_UNITS, TELEPORT_GIVE_UP_MS, TeleportCheck, check_teleport,
};

pub const FOLLOW_RETRY_MS: u64 = 1500;
pub const TOGETHER_FREE_TO_ACT_GRACE_MS: u64 = 600;
pub const FOLLOW_WAITS_FOR_THE_SCENE_END_MS: u64 = 5000;
pub const ANNOUNCE_RESEND_MS: u64 = 2000;
pub const DEFAULT_LEASH_UNITS: f32 = 40.0;
pub const MIN_LEASH_UNITS: f32 = 5.0;
pub const MAX_LEASH_UNITS: f32 = 500.0;
pub const ARRIVAL_PLACEMENT_TRIES: u32 = 20;
pub const ARRIVAL_PLACEMENT_RETRY_EVERY_MS: u64 = 250;
pub const SCRIPT_JUMP_MIN_UNITS: f32 = 15.0;
pub const SCRIPT_JUMP_PARTNER_NEAR_UNITS: f32 = 15.0;
pub const SCRIPT_JUMP_SETTLE_MS: u64 = 1500;
pub const PARTNER_TIMEOUT_MS: u64 = 3000;
pub const SPOT_RING_RADII: [f32; 3] = [1.5, 2.5, 3.5];
pub const SPOT_ANGLE_STEPS: u32 = 8;
pub const MAX_FLOOR_STEP_FROM_PARTNER: f32 = 1.0;
pub const MIN_FLOOR_NORMAL_Y: f32 = 0.7;
pub const FLOOR_RAY_START_ABOVE_PARTNER: f32 = 2.0;
pub const FLOOR_RAY_END_BELOW_PARTNER: f32 = 6.0;
pub const BODY_RADIUS: f32 = 0.7;
pub const BODY_FLOOR_CLEARANCE: f32 = 0.1;
