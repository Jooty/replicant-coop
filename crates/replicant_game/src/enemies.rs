mod aggro;
mod attack_record;
mod boss_bar_probe;
mod boss_fight;
mod boss_hit_log;
mod bullet_credit;
mod ca_gauge;
mod companion;
mod copy_hold;
mod copy_stance;
mod count_log;
mod creation;
mod difficulty;
mod dying_bodies;
mod enemy_log;
mod fight_binding;
mod ghosts;
mod guest_only;
mod hit_gate;
mod hit_guard;
mod hit_kind;
mod hit_log_limit;
mod hit_reaction;
mod hit_stagger;
mod host_id_list;
mod host_removal;
mod host_rounds;
mod host_view;
mod id_list;
mod interval_clock;
mod kill_watch;
mod list_diff;
mod matching;
mod mirror;
mod own_hit_log;
mod partners;
mod player_hit_guard;
mod removal_cause;
mod roster;
mod scan;
mod shadow_matrix;
mod sighting;
mod slide_probe;
mod snapshot;
mod takeover;
mod time_scale;

pub use aggro::{AggroPlanner, AggroPlayers, AggroTarget, choose_target};
pub use attack_record::{ActorHandle, AttackRecord};
pub use boss_bar_probe::{
    BOSS_BAR_DUMP_BYTES, BOSS_BAR_DUMP_INTERVAL_MS, BOSS_BAR_INIT_FUNCTION_RVA,
    BOSS_BAR_SCRIPT_COMMAND_RVA, BossBarDump, boss_bar_dump_text, boss_bar_init_text,
    boss_bar_script_text, boss_bar_state_is_open, read_boss_bar_dump_text, read_boss_bar_handle,
    read_boss_bar_open, read_script_argument, script_argument_value,
};
pub use boss_fight::{
    BOSS_HP_LOG_INTERVAL_MS, BossFight, BossFightChange, BossPick, boss_fight_hp_text, boss_hp_text,
};
pub use boss_hit_log::{BossHitLog, BossHitPlayer};
pub use bullet_credit::credited_enemy;
pub use ca_gauge::{
    CA_GAUGE_CHANGE_LINES_PER_SECOND, CA_GAUGE_ENTRIES_RVA, CA_GAUGE_ENTRY_COUNT,
    CA_GAUGE_ENTRY_SIZE, CA_GAUGE_HEARTBEAT_INTERVAL_MS, CA_GAUGE_MANAGER_RAW_BYTES,
    CA_GAUGE_MANAGER_RVA, CA_GAUGE_SCAN_INTERVAL_MS, CaGaugeEntry, CaGaugeEvent, CaGaugeRaw,
    CaGaugeScanStats, CaGaugeWatch, ca_entry_for_actor, ca_gauge_heartbeat_text, ca_gauge_hit_text,
    ca_gauge_probe_on_text, read_ca_gauge_manager_raw,
};
pub use companion::{
    CompanionControl, CompanionControlLog, CompanionGuestInputs, CompanionOwnLogicCause,
    CompanionReading, companion_control, companion_control_text, companion_hit_counts,
    companion_name, host_sends_companion, read_companions,
};
pub use copy_hold::{holds_copy, kept_step, stop_own_step, stop_own_velocity};
pub use copy_stance::{CopyStance, CopyStanceLog};
pub use count_log::CountLog;
pub use creation::{CreationWatch, missing_host_spawns, spec_slot_address, spec_slot_index};
pub use difficulty::{
    DamageTakenLedger, EXTRA_PLAYERS_WITH_ONE_GUEST, GuestHitDamage, ScaledEnemyKey,
    damage_dealt_bonus_percent, damage_taken_factor, guest_hit_damage, scaled_hp_damage,
};
pub use dying_bodies::DyingBodies;
pub use enemy_log::{EnemyLog, relayed_spawn_text, sighting_text};
pub use fight_binding::{FightBindingPlan, HostFightEnemy, WaitingFightSpawn, plan_fight_bindings};
pub use ghosts::{GhostPlan, ghost_is_same_spawn, plan_ghosts};
pub use guest_only::GuestOnlyWatch;
pub use hit_gate::victim_refuses_hit;
pub use hit_guard::{GuestHitGuard, HitVerdict};
pub use hit_kind::{pack_hit_kind, unpack_hit_kind};
pub use hit_log_limit::EnemyHitLogLimiter;
pub use hit_reaction::{HitReaction, player_reaction_kind};
pub use hit_stagger::{
    DAMAGE_STATE_NAME, HostHitStaggerWatch, PendingStaggerCheck, StaggerVerdict,
    player_reaction_motion, read_player_state_name,
};
pub use host_removal::host_removal_destroys_copy;
pub use host_rounds::CompleteRound;
pub use host_view::{HostHas, HostListView};
pub use id_list::split_id_list;
pub use interval_clock::IntervalClock;
pub use kill_watch::KillWatch;
pub use list_diff::{ListDiff, ListDiffWatch, LocalKeep};
pub use matching::{EnemyMatch, match_enemies};
pub use mirror::{EnemyMirror, MirroredEnemy};
pub use own_hit_log::{
    OWN_HITS_LOGGED_EACH_PER_PHASE, OWN_HITS_LOGGED_EVERY_NTH_AFTER, OwnHitInputs, OwnHitLimiter,
    OwnHitRecord, own_hit_text, spec_class_of,
};
pub use partners::{live_companion_texts, partner_actors, unique_companion_pose};
pub use player_hit_guard::PlayerHitGuard;
pub use removal_cause::RemovalCause;
pub use roster::{EnemyRoster, RosterUpdate};
pub(crate) use scan::read_group_counts;
pub use scan::{read_group_count, scan_enemies};
pub use shadow_matrix::{creation_matrix_scale, scaled_matrix, shadow_matrix};
pub use sighting::EnemySighting;
pub use slide_probe::{SlideReading, horizontal_distance, read_slide_reading, slide_check_text};
pub use snapshot::{ENEMY_ROUND_CAPACITY, enemy_state, split_snapshot};
pub use takeover::{
    FIGHT_SPAWN_FILL_IN_GRACE_MS, FIGHT_SPAWN_FILL_IN_RETRIES, FIGHT_SPAWN_FILL_IN_RETRY_GAP_MS,
    FightFillInStep, FightSpawnFillIn, fight_spawn_fill_in_gave_up_text,
    fight_spawn_fill_in_snap_off_text, fight_spawn_filled_in_text, plan_takeovers,
};
pub use time_scale::{NORMAL_TIME_SCALE, restored_time_scale, stalled_copy_time_scale_fix};

pub const ENEMY_MIRROR_STALE_MS: u64 = 1000;
pub const HOST_HIT_STAGGER_CHECK_WINDOW_MS: u64 = 500;
pub const ENEMY_ID_LIST_STALE_MS: u64 = 4000;
pub const ENEMY_ID_LIST_INTERVAL_MS: u64 = 1000;
pub const ENEMY_COUNT_LOG_INTERVAL_MS: u64 = 2000;
pub const ENEMY_MIRROR_DELAY_MS: u64 = 100;
pub const AGGRO_PUSH_RANGE: f32 = 15.0;
pub const AGGRO_SWITCH_MARGIN: f32 = 1.5;
pub const AGGRO_REFRESH_MS: u64 = 2000;
pub const AGGRO_PUSH_WEIGHT: f32 = 100.0;
pub const GUEST_HIT_REPEAT_WINDOW_MS: u64 = 2000;
pub const PLAYER_HIT_REPEAT_WINDOW_MS: u64 = 2000;
pub const GUEST_ONLY_ENEMY_GRACE_MS: u64 = 3000;
pub const FIGHT_GUEST_ONLY_ENEMY_GRACE_MS: u64 = 1000;
pub const HOST_SPAWN_COPY_GRACE_MS: u64 = 3000;
pub const ENEMY_LIST_DIFF_INTERVAL_MS: u64 = 5000;
pub const SMALLEST_OWN_STEP_WORTH_LOGGING: f32 = 0.01;
pub const LARGEST_STORY_ACTOR_MAX_HP: i32 = 1;
