use replicant_game::save_files::CoopSaveName;

use crate::campaign_flow::{
    MAX_CAMPAIGN_ID_CHARACTERS, MIN_CAMPAIGN_ID_CHARACTERS, SESSION_FILE_ID_CHARACTERS,
    SESSION_FILE_PREFIX,
};

/// The campaign id when it is safe to put in a file name: hex digits only, of a sane length.
pub fn campaign_file_stem(campaign_id: &str) -> Option<&str> {
    let length_ok =
        (MIN_CAMPAIGN_ID_CHARACTERS..=MAX_CAMPAIGN_ID_CHARACTERS).contains(&campaign_id.len());
    (length_ok
        && campaign_id
            .chars()
            .all(|character| character.is_ascii_hexdigit()))
    .then_some(campaign_id)
}

/// The guest's session file for a campaign, a name the mod accepts as its co-op file.
pub fn session_file_name(campaign_id: &str) -> Option<String> {
    let stem = campaign_file_stem(campaign_id)?;
    let name = format!(
        "{SESSION_FILE_PREFIX}{}",
        stem[..SESSION_FILE_ID_CHARACTERS].to_ascii_lowercase()
    );
    CoopSaveName::parse(Some(&name)).ok()?;
    Some(name)
}
