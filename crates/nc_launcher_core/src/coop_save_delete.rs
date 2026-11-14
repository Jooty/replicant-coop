mod coop_save_files;
mod delete_coop_saves;
mod delete_offer;

use replicant_game::save_files::COOP_GAMEDATA_FILE_NAME;

use crate::campaign_flow::SESSION_FILE_PREFIX;

pub const COOP_SAVE_NAME_PREFIXES: [&str; 2] = [COOP_GAMEDATA_FILE_NAME, SESSION_FILE_PREFIX];
pub const DELETED_SAVES_FOLDER_SUFFIX: &str = "-deleted";
pub const DELETED_SAVES_FOLDERS_KEPT: usize = 3;

pub use coop_save_files::{coop_saves_in, is_coop_save_name};
pub use delete_coop_saves::{
    CoopSaveDeleteError, DeletedCoopSaves, delete_coop_saves, keep_newest_deleted_folders,
};
pub use delete_offer::{CoopSaveDeleteOffer, coop_save_delete_offer};
