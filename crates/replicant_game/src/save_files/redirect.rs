use crate::save_files::{CoopSaveName, GAMEDATA_FILE_NAME};

pub fn coop_save_name<'name>(
    game_file_name: &[u8],
    coop: &'name CoopSaveName,
) -> Option<&'name [u8]> {
    (game_file_name == GAMEDATA_FILE_NAME.as_bytes()).then_some(coop.with_nul())
}
