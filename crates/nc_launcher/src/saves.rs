mod backup;
mod campaign_backup;
mod campaign_drop;
mod campaign_id;
mod campaign_seat;
mod campaigns;
mod coop;
mod fresh;
mod guest_seat;
mod host_campaign;
mod location;
mod restore;
mod seats;
mod slots;
mod write_atomic;

pub use backup::{BackupError, backup_saves, copy_dir_recursive, format_stamp, stamp_label};
pub use campaign_drop::{
    SetAside, drop_campaign, drop_lost_campaigns, host_slots_that_lost_their_campaign,
};
pub use campaign_id::{campaign_id_from, new_campaign_id};
pub use campaign_seat::{read_campaign_seat, store_campaign_seat};
pub use campaigns::{
    Campaign, CampaignRecord, SeatSummary, coop_campaigns_path, parse_campaigns, read_campaigns,
    render_campaigns, store_campaign, write_campaigns,
};
pub use coop::{
    COOP_BACKUP_FOLDER, COOP_BEFORE_RESTORE_PREFIX, CoopTargetError, backup_coop_file,
    coop_copy_target_slot, coop_gamedata_path, copy_slot_into_coop, first_empty_slot, merge_slot,
    read_coop_slots, restore_coop_backup,
};
pub use fresh::fresh_coop_file;
pub use guest_seat::{
    PreparedSession, hidden_copy_path, keep_unsent, newest_seat_in, play_seconds_of,
    prepare_session_file, promote_delivered, read_hidden_copy, read_session_file, read_unsent,
    seat_backups_dir, seats_in, unsent_copy_path,
};
pub use host_campaign::{
    ArrivedSeat, campaign_card_of, campaign_for_host_slot, keep_arrived_seat_aside,
    seat_summary_of, store_arrived_seat,
};
pub use location::{backup_root, coop_backup_root, save_dir, save_folders};
pub use replicant_game::save_files::GAMEDATA_FILE_NAME;
pub use restore::{PRE_RESTORE_PREFIX, list_backups, restore_backup};
pub use seats::{
    SeatFileError, coop_seats_dir, finish_staged_seat, read_seat, seat_file_path,
    set_aside_seats_dir, set_seat_aside, stage_seat, store_seat,
};
pub use slots::{
    SlotError, SlotSummary, default_slot, gamedata_path, parse_slots, play_time_short,
    play_time_text, read_slots, slot_label, slot_player_name, summarize_seat,
};
pub use write_atomic::{unix_seconds_now, unused_path, write_atomically};

pub const MY_GAMES_FOLDER: &str = "My Games";
pub const BACKUP_FOLDER: &str = "save-backups";
pub const PROTON_SAVE_PREFIX_SEGMENTS: [&str; 3] = ["drive_c", "users", "steamuser"];
pub const STEAM_SAVE_FOLDER: &str = "Steam";
pub const GAMEDATA_SIZE: usize = 0x48200;
pub const SLOT_COUNT: u8 = 7;
pub const SLOTS_START: usize = 0x8160;
pub const SLOT_STRIDE: usize = 0x9260;
pub const HEADER_SLOT_MASK_OFFSET: usize = 0x6C;
pub const HEADER_ENDING_RECORD_RANGES: [std::ops::Range<usize>; 3] =
    [0x04..0x08, 0x50..0x57, 0x176..0x1A5];
pub const SLOT_VERSIONS: [u32; 2] = [200, 0x84];
pub const SLOT_VERSION_OFFSET: usize = 0x00;
pub const SLOT_PHASE_OFFSET: usize = 0x04;
pub const SLOT_PHASE_LENGTH: usize = 0x20;
pub const SLOT_NAME_OFFSET: usize = 0x2C;
pub const SLOT_NAME_LENGTH: usize = 0x20;
pub const SLOT_LEVEL_OFFSET: usize = 0x64;
pub const SLOT_PLAYTIME_OFFSET: usize = 0x4A0;
