mod arrays;
mod changed_slot;
mod ending_session;
mod feed;
mod header;
mod milestones;
mod mirror;
mod player_local_flags;
mod rank;
mod save_rule;
mod seat;
mod sidecar;
mod slot_story;

pub(crate) use arrays::fnv_hash;
pub use arrays::{StoryArrays, StoryKey};
pub use changed_slot::changed_slot;
pub use ending_session::EndingRecordsSession;
pub use feed::{FeedSend, StoryFeed};
pub use header::{
    HeaderTooShort, apply_header_records, ending_records_are_empty, ending_records_gained,
    fix_header_checksum, header_checksum, merge_ending_records, read_ending_records,
};
pub use milestones::milestone_ids;
pub use mirror::{StoryMirror, StoryPlan};
pub use player_local_flags::{is_player_local_flag, without_player_local_flags};
pub use rank::{StoryRank, part_of_phase};
pub use save_rule::{
    SlotOutcome, StoryRule, StorySource, apply_story_rule, best_story_for_name, save_file_is_valid,
};
pub use seat::{SeatError, check_seat, seat_bytes_of, session_file_with_seat};
pub use sidecar::{decode_high_water_sidecar, encode_high_water_sidecar};
pub use slot_story::{SlotStory, fix_slot_checksum, slot_checksum, slot_player_name};

pub const GAME_FLAGS_BLOCK_OFFSET: usize = 0x500;
pub const GAME_COUNTERS_BLOCK_OFFSET: usize = 0x600;
pub const KEY_FRAGMENTS_BLOCK_OFFSET: usize = 0xBA8;
pub const SNOW_FLAGS_BLOCK_OFFSET: usize = 0xC30;
pub const SNOW_COUNTERS_BLOCK_OFFSET: usize = 0xD30;
pub const PHASE_NAME_BLOCK_OFFSET: usize = 0x04;
pub const SAVE_POINT_BLOCK_OFFSET: usize = 0x24;
pub const PLAYER_NAME_BLOCK_OFFSET: usize = 0x2C;
pub const PLAYER_NAME_BYTES: usize = 0x20;
pub const STORY_BLOCK_READ_BYTES: usize = 0xE30;
pub const SLOT_VERSION_OFFSET: usize = 0;
pub const SLOT_VERSIONS_IN_USE: [u32; 2] = [200, 0x84];
pub const SLOT_CHECKSUM_OFFSET: usize = 0x9250;
pub const SLOT_CHECKSUM_COVERED_BYTES: usize = 0xC20;
pub const SLOT_BYTES: usize = 0x9260;
pub const SLOTS_FILE_OFFSET: usize = 0x8160;
pub const SAVE_FILE_SLOT_COUNT: usize = 7;
pub const SAVE_FILE_SLOT_MASK_OFFSET: usize = 0x6C;
pub const SAVE_FILE_BYTES: usize = 0x4_8200;
pub const GAME_FLAG_SET_FUNCTION_RVA: u32 = 0x3B_0240;
pub const GAME_FLAG_CLEAR_FUNCTION_RVA: u32 = 0x3B_0210;
pub const SNOW_FLAG_SET_FUNCTION_RVA: u32 = 0x3B_1CC0;
pub const SNOW_FLAG_CLEAR_FUNCTION_RVA: u32 = 0x3B_1C90;
pub const GAME_COUNTER_SET_FUNCTION_RVA: u32 = 0x3B_1270;
pub const SNOW_COUNTER_SET_FUNCTION_RVA: u32 = 0x3B_18D0;
pub const STORY_DRIFT_GRACE_MS: u64 = 2_000;
pub const STORY_DRIFT_KEYS_THAT_MEAN_A_RELOAD: usize = 64;
pub const STORY_CHANGES_BEFORE_A_FULL_RESEND: usize = 160;
pub const STORY_DIGEST_INTERVAL_MS: u64 = 2_000;
pub const LAST_LOADED_SLOT_RVA: u32 = 0x120_7284;
pub const ROUTE_CYCLE_FLAG_IDS: [u16; 4] = [918, 919, 920, 921];
pub const SAME_STORY_REQUEST_IGNORED_MS: u64 = 5_000;
pub const STORY_MESSAGES_KEPT_BEFORE_ARMING: usize = 512;
pub const STORY_REQUEST_RETRY_MS: u64 = 5_000;
pub const RANK_SIDECAR_MAGIC: [u8; 4] = *b"RCRK";
pub const RANK_SIDECAR_VERSION: u8 = 2;
pub const RANK_SIDECAR_SUFFIX: &str = ".rank";
pub const HEADER_BYTES: usize = 0x8158;
pub const HEADER_CHECKSUM_OFFSET: usize = 0x8154;
pub const ENDING_CLEAR_BITS_HEADER_OFFSET: usize = 0x04;
pub const ENDING_COSTUMES_HEADER_OFFSET: usize = 0x50;
pub const ENDING_ACCOMPLISHED_HEADER_OFFSET: usize = 0x176;
pub const ENDING_RECORDS_LIVE_READ_BYTES: usize = 0x1A5;
pub const LIVE_HEADER_RVA: u32 = 0x11F_DF80;
pub const FINISH_HOUR_ACCOMPLISHED_INDEX: usize = 0x21;
