use std::path::Path;

use nc_hook::net::{game_folder, parse_env_flag, parse_env_number, read_config_text};
use serde::Deserialize;

pub use nc_hook::net::{
    AUTO_CONFIG_FILE_NAME, ConfigError as AutoConfigError, DEFAULT_LOG_FILE_NAME, LINK_ENV,
    LOG_ENV, NAME_ENV, resolve_log_path,
};

pub const BOOT_SLOT_ENV: &str = "NIER_COOP_BOOT_SLOT";
pub const NEW_GAME_SLOT_ENV: &str = "NIER_COOP_NEW_GAME_SLOT";
pub const NEW_GAME_NAME_ENV: &str = "NIER_COOP_NEW_GAME_NAME";
pub const BLOCK_SAVES_ENV: &str = "NIER_COOP_BLOCK_SAVES";
const ALL_ENV_KEYS: [&str; 7] = [
    LINK_ENV,
    NAME_ENV,
    BOOT_SLOT_ENV,
    NEW_GAME_SLOT_ENV,
    NEW_GAME_NAME_ENV,
    BLOCK_SAVES_ENV,
    LOG_ENV,
];
const SAVE_SLOT_COUNT: u8 = 32;
const NEW_GAME_SLOT_COUNT: u8 = 6;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoConfig {
    pub link: Option<u16>,
    pub name: Option<String>,
    pub boot_slot: Option<u8>,
    pub new_game_slot: Option<u8>,
    pub new_game_name: Option<String>,
    #[serde(default)]
    pub block_saves: bool,
    #[serde(default)]
    pub keep_running: bool,
    #[serde(default)]
    pub dev_no_focus_menu: bool,
    pub log: Option<String>,
    #[serde(default)]
    pub show_distance: bool,
    #[serde(default)]
    pub show_ping: bool,
    pub event_log: Option<bool>,
    pub show_names: Option<bool>,
    pub show_party_frame: Option<bool>,
    pub enemy_health_percent: Option<u16>,
    pub enemy_damage_percent: Option<u16>,
    #[serde(rename = "body")]
    pub retired_body_key: Option<String>,
    pub coop_save_file: Option<String>,
    pub companion_rows_from_scripts: Option<bool>,
}

impl AutoConfig {
    pub fn parse(text: &str) -> Result<Self, AutoConfigError> {
        toml::from_str::<Self>(text)
            .map_err(|error| AutoConfigError::Invalid(error.to_string()))?
            .validated()
    }

    fn validated(self) -> Result<Self, AutoConfigError> {
        if self.link == Some(0) {
            return Err(AutoConfigError::Invalid("link must not be 0".to_owned()));
        }
        if self.boot_slot.is_some_and(|slot| slot >= SAVE_SLOT_COUNT) {
            return Err(AutoConfigError::Invalid(format!(
                "boot_slot must be 0 to {}",
                SAVE_SLOT_COUNT - 1
            )));
        }
        if self
            .new_game_slot
            .is_some_and(|slot| slot >= NEW_GAME_SLOT_COUNT)
        {
            return Err(AutoConfigError::Invalid(format!(
                "new_game_slot must be 0 to {}",
                NEW_GAME_SLOT_COUNT - 1
            )));
        }
        if self.new_game_slot.is_some() && self.boot_slot.is_some() {
            return Err(AutoConfigError::Invalid(
                "new_game_slot and boot_slot cannot both be set".to_owned(),
            ));
        }
        Ok(self)
    }

    pub fn event_log_enabled(&self) -> bool {
        self.event_log.unwrap_or(true)
    }

    /// The companion rows generated from the script packs take part unless this is set false.
    pub fn companion_rows_from_scripts_enabled(&self) -> bool {
        self.companion_rows_from_scripts.unwrap_or(true)
    }

    pub fn load_beside_game() -> Result<Option<Self>, AutoConfigError> {
        match game_folder() {
            Some(folder) => Self::load_layered(&folder, |key| std::env::var(key).ok()),
            None => Self::layered(None, |key| std::env::var(key).ok()),
        }
    }

    pub fn load_layered(
        folder: &Path,
        environment: impl Fn(&str) -> Option<String>,
    ) -> Result<Option<Self>, AutoConfigError> {
        Self::layered(Self::load_from_folder(folder)?, environment)
    }

    pub fn layered(
        file: Option<Self>,
        environment: impl Fn(&str) -> Option<String>,
    ) -> Result<Option<Self>, AutoConfigError> {
        if file.is_none() && ALL_ENV_KEYS.iter().all(|key| environment(key).is_none()) {
            return Ok(None);
        }
        let mut config = file.unwrap_or_default();
        if let Some(text) = environment(LINK_ENV) {
            config.link = Some(parse_env_number(LINK_ENV, &text)?);
        }
        if let Some(text) = environment(NAME_ENV) {
            config.name = Some(text);
        }
        if let Some(text) = environment(BOOT_SLOT_ENV) {
            config.boot_slot = Some(parse_env_number(BOOT_SLOT_ENV, &text)?);
        }
        if let Some(text) = environment(NEW_GAME_SLOT_ENV) {
            config.new_game_slot = Some(parse_env_number(NEW_GAME_SLOT_ENV, &text)?);
        }
        if let Some(text) = environment(NEW_GAME_NAME_ENV) {
            config.new_game_name = Some(text);
        }
        if let Some(text) = environment(BLOCK_SAVES_ENV) {
            config.block_saves = parse_env_flag(BLOCK_SAVES_ENV, &text)?;
        }
        if let Some(text) = environment(LOG_ENV) {
            config.log = Some(text);
        }
        config.validated().map(Some)
    }

    pub fn load_from_folder(folder: &Path) -> Result<Option<Self>, AutoConfigError> {
        read_config_text(folder)?
            .map(|text| Self::parse(&text))
            .transpose()
    }
}
