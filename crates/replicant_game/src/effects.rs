mod anchor_offset;
mod capture;
mod falloff;
mod hit_spark;
mod instance;
mod live_effects;
mod look;
mod name_tally;
mod probe_log;
mod push;
mod rate_window;
mod sent_effects;
mod source;
mod voice_character;
mod world_trail;

pub use anchor_offset::{match_anchor_offset, read_anchor_offset};
pub use capture::{
    CapturedEffect, EffectCall, WORLD_EFFECT_SENT_PREFIX, build_effect_call, read_effect,
    read_sound, read_world_effect,
};
pub use falloff::sound_gain;
pub use hit_spark::{
    HitSparkNames, hit_spark_log_line, spawn_is_drawn_by_shot_sync, spawn_is_sent,
};
pub use instance::{
    effect_instance_address, effect_is_finished, finish_effect, read_effect_transform,
};
pub use live_effects::LiveEffects;
pub use look::{
    EFFECT_LOOK_RETRY_FRAMES, LookAttempt, OriginNames, look_attempt, look_is_set, look_log_line,
    look_needs_write, read_effect_look, write_effect_look,
};
pub use name_tally::{EffectNameTally, NameCount, name_counts_text, world_effects_shown_line};
pub use probe_log::{PROBE_LOG_LINES_PER_WINDOW, PROBE_LOG_WINDOW_MS, ProbeLogLimiter};
pub use push::{
    AFTER_OWN_HIT_WINDOW_MS, DROP_LOG_INTERVAL_MS, DropLogLimiter, FLUSHER_HIT_RADIUS,
    MAX_RECENT_PUSHED_HANDLES, PendingAdmission, PushCaller, RecentHandles, admit_pending,
    census_log_line, classify_push_caller, push_is_sent, read_request_id, read_request_name,
    read_request_transform, within_after_hit_window, within_hit_radius,
};
pub use rate_window::RateWindow;
pub use sent_effects::SentEffects;
pub use source::{
    EffectSource, POWER_ACTOR_CLASSES, PowerActorClass, SoundRoute, classify_effect_source,
    is_normal_bullet, local_book_actor, local_has_book, power_actor_class, power_actor_owner,
    projectile_kind, projectile_owner, read_actor_position,
};
pub use voice_character::{
    FirstTimeNames, VOICE_SWITCH_GROUP, VoiceCharacterSchedule, read_character_state,
    wwise_name_hash,
};
pub use world_trail::{
    MAX_TRAILED_WORLD_EFFECTS, WORLD_EFFECT_MOVE_EPSILON, WORLD_EFFECT_MOVE_INTERVAL_MS,
    WorldEffectTrail,
};

pub const SOUND_FALLOFF_NEAR_UNITS: f32 = 6.0;
pub const SOUND_FALLOFF_FAR_UNITS: f32 = 45.0;
pub const MAX_LIVE_PUPPET_EFFECTS: usize = 64;
pub const MAX_LASTING_SENT_EFFECTS: usize = 256;
pub const PUPPET_EFFECT_MAX_LIFETIME_MS: u64 = 8000;
pub const MAX_EFFECT_EVENTS_PER_SECOND: u32 = 40;
pub const MAX_PUPPET_EFFECTS_PER_TICK: usize = 8;
pub const MAX_EFFECT_NAMES_IN_A_SUMMARY: usize = 8;
pub const EFFECT_LOG_LINES_PER_WINDOW: u32 = 12;
pub const EFFECT_LOG_WINDOW_MS: u64 = 5000;
