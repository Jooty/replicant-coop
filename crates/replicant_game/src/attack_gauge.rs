mod boss_gauge_state;
mod guest_hits;
mod guest_plan;
mod host_ask;
mod host_hits;
mod host_watch;
mod layout;
mod log_text;
mod off_switch;
mod opening_hit;
mod tally;

pub use boss_gauge_state::{BossGaugeState, boss_gauge_state_text};
pub use guest_hits::{GuestGaugeHits, measured_gauge_damage};
pub use guest_plan::{
    GuestGaugeFacts, GuestGaugeStep, GuestGaugeWait, HostGaugeView, LocalGaugeView,
    guest_gauge_step, guest_node_hp, guest_node_hp_after_own_hit, guest_remaining_floor,
};
pub use host_ask::{GaugeAskFacts, GaugeAskRefusal, GaugeAskVerdict, HostGaugeAsks};
pub use host_hits::{
    GaugeHitDrop, GaugeHitFacts, GaugeHitVerdict, HostGaugeHits, node_hp_after_damage,
};
pub use host_watch::HostGaugeWatch;
pub use layout::{
    GaugeNodeReading, GaugeReading, count_nodes, manager_address, read_gauge,
    read_hp_floor_percent, write_gauge_clock, write_node_hp,
};
pub use log_text::{
    gauge_close_text, gauge_hit_applied_text, gauge_hit_dropped_text, gauge_open_text,
    gauge_tally_text, guest_ask_sent_text, guest_close_text, guest_open_readback_text,
    guest_open_text, guest_wait_text, host_ask_gave_up_text, host_ask_refused_text,
    host_ask_replayed_text, node_break_text, own_gauge_hit_text,
};
pub use off_switch::{GaugeOffSwitch, attack_gauge_boot_text, attack_gauge_off_file};
pub use opening_hit::{
    GuestOpeningHits, OpeningHitMiss, OpeningHitReplay, OpeningHitStep, OpeningReplayReport,
};
pub use tally::{GaugeSwingMiss, GaugeTally};

pub const ATTACK_GAUGE_MANAGER_RVA: u32 = 0x11A_4240;
pub const ATTACK_GAUGE_HP_FLOOR_PERCENT_RVA: u32 = 0x11A_F328;
pub const ADD_ATTACK_GAUGE_NODE_RVA: u32 = 0x36_C660;
pub const OPEN_ATTACK_GAUGE_RVA: u32 = 0x36_F580;
pub const OPEN_TWO_ACTOR_ATTACK_GAUGE_RVA: u32 = 0x36_F0F0;
pub const CLOSE_ATTACK_GAUGE_RVA: u32 = 0x36_CF10;
pub const UPDATE_ATTACK_GAUGE_RVA: u32 = 0x36_D220;
pub const ATTACK_GAUGE_DUMMY_HIT_RVA: u32 = 0x36_EA80;
pub const BREAK_ATTACK_GAUGE_NODE_RVA: u32 = 0x36_CC70;
pub const ATTACK_GAUGE_MODE_OFFSET: u64 = 0x0;
pub const ATTACK_GAUGE_BOSS_HANDLE_OFFSET: u64 = 0x4;
pub const ATTACK_GAUGE_PARTNER_HANDLE_OFFSET: u64 = 0xC;
pub const ATTACK_GAUGE_TOTAL_SECONDS_OFFSET: u64 = 0x14;
pub const ATTACK_GAUGE_REMAINING_SECONDS_OFFSET: u64 = 0x18;
pub const ATTACK_GAUGE_DUMMY_HANDLE_OFFSET: u64 = 0x1C;
pub const ATTACK_GAUGE_FIRST_NODE_OFFSET: u64 = 0x40;
pub const ATTACK_GAUGE_MAP_POINTS_OFFSET: u64 = 0xB0B8;
pub const ATTACK_GAUGE_OPTIONS_OFFSET: u64 = 0xB0C4;
pub const ATTACK_GAUGE_NODE_BONE_OFFSET: u64 = 0x0;
pub const ATTACK_GAUGE_NODE_SIZE_OFFSET: u64 = 0x4;
pub const ATTACK_GAUGE_NODE_OFFSET_OFFSET: u64 = 0x10;
pub const ATTACK_GAUGE_NODE_KIND_OFFSET: u64 = 0x3C;
pub const ATTACK_GAUGE_NODE_EXTRA_OFFSET: u64 = 0x40;
pub const ATTACK_GAUGE_NODE_HP_OFFSET: u64 = 0x44;
pub const ATTACK_GAUGE_NODE_MAX_HP_OFFSET: u64 = 0x48;
pub const ATTACK_GAUGE_NODE_NEXT_OFFSET: u64 = 0x50;
pub const BOSS_ATTACK_GAUGE_STATE_OFFSET: u64 = 0x2_4E60;
pub const BOSS_ATTACK_GAUGE_BREAKS_LEFT_OFFSET: u64 = 0x2_4E64;
pub const BOSS_ATTACK_GAUGE_STATE_OPEN: i32 = 1;
pub const BOSS_ATTACK_GAUGE_STATE_BROKEN: i32 = 2;
pub const BOSS_ATTACK_GAUGE_STATE_TIMED_OUT: i32 = 3;
pub const ATTACK_GAUGE_MODE_ONE_ACTOR: i32 = 1;
pub const ATTACK_GAUGE_MODE_TWO_ACTORS: i32 = 2;
pub const ATTACK_GAUGE_PERCENT_DIVISOR: f32 = 100.0;
pub const GUEST_GAUGE_REMAINING_FLOOR_SECONDS: f32 = 0.5;
pub const GUEST_ONLY_GAUGE_GRACE_MS: u64 = 1500;
pub const GUEST_ASKED_GAUGE_GRACE_MS: u64 = 4000;
pub const HOST_REFUSES_GAUGE_ASK_AFTER_CLOSE_MS: u64 = 3000;
pub const HOST_REPLAYS_GUEST_HIT_AT_MOST_MS_BEFORE_ASK: u64 = 1500;
pub const HOST_WAITS_FOR_GUEST_OPENING_HIT_MS: u64 = 500;
pub const ATTACK_GAUGE_TALLY_INTERVAL_MS: u64 = 1000;
pub const HOST_GAUGE_RESEND_INTERVAL_MS: u64 = 250;
pub const GUEST_GAUGE_HIT_REPEAT_WINDOW_MS: u64 = 2000;
pub const GAUGE_HIT_SEQ_MEMORY_ENTRIES: usize = 128;
pub const GUEST_GAUGE_MEASURING_HP: i32 = 1_000_000;
pub const ATTACK_GAUGE_OFF_FILE_NAME: &str = "attack-gauge-off";
pub const ATTACK_GAUGE_SWITCH_CHECK_INTERVAL_MS: u64 = 1000;
pub const ATTACK_GAUGE_SYNC_ON_TEXT: &str = "attack gauge: sync on";
pub const ATTACK_GAUGE_SYNC_OFF_TEXT: &str =
    "attack gauge: sync off (attack-gauge-off file present)";
pub const ATTACK_GAUGE_SWITCHED_OFF_TEXT: &str = "attack gauge: switched off by the attack-gauge-off file, the game's own gauge runs on each game";
