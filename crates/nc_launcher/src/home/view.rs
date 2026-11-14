use std::path::Path;

use nc_launcher_core::banner_facts;

use crate::home::GameCard;
use crate::install::ModStatus;

const INSTALL_MOD_LABEL: &str = "Install the mod";
const UPDATE_READY: &str = "update ready";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardTone {
    Good,
    Warn,
    Bad,
    Muted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardView {
    pub title: &'static str,
    pub tags: &'static [&'static str],
    pub found_text: String,
    pub found_tone: CardTone,
    pub folder_display: Option<String>,
    pub mod_text: String,
    pub mod_tone: CardTone,
    pub update_text: Option<String>,
    pub update_from_server: bool,
    pub greyed_out: bool,
    pub play_enabled: bool,
    pub offers_install: bool,
    pub mod_action: Option<&'static str>,
}

/// What a game's home row shows. `server_build` is the short commit of a build the update server offers.
pub fn card_view(card: &GameCard, server_build: Option<&str>) -> CardView {
    let found = card.install.as_ref().filter(|game| game.exe_present());
    let (found_text, found_tone) = match (&card.install, found) {
        (_, Some(game)) if card.exe_supported => {
            (game.game_dir.display().to_string(), CardTone::Good)
        }
        (_, Some(game)) => (game.game_dir.display().to_string(), CardTone::Bad),
        (Some(game), None) => (
            format!("No game executable in {}", game.game_dir.display()),
            CardTone::Bad,
        ),
        (None, None) => ("Not found".to_owned(), CardTone::Bad),
    };
    let (mod_text, mod_tone) = mod_line(card);
    let offers_install = card.profile.support.install && found.is_some();
    let server_update =
        server_build.filter(|_| card.mod_status == ModStatus::Current && offers_install);
    // An update is offered by its own line on the row, so the only separate action is installing.
    let mod_action =
        (card.mod_status == ModStatus::NotInstalled && offers_install).then_some(INSTALL_MOD_LABEL);
    let update_text = match card.mod_status {
        ModStatus::Outdated if card.profile.support.install => Some(
            card.ready_build
                .as_deref()
                .map_or_else(|| UPDATE_READY.to_owned(), build_ready_line),
        ),
        _ => server_update.map(build_ready_line),
    };
    let facts = banner_facts(card.profile.game_id);
    CardView {
        title: card.profile.display_name,
        tags: facts.tags,
        folder_display: found
            .map(|game| folder_display(&game.game_dir, dirs::home_dir().as_deref())),
        found_text,
        found_tone,
        mod_text,
        mod_tone,
        update_text,
        update_from_server: server_update.is_some(),
        greyed_out: found.is_none(),
        play_enabled: found.is_some(),
        offers_install,
        mod_action,
    }
}

/// The folder from the home directory (as `~`), on one line.
pub fn folder_display(game_dir: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| game_dir.strip_prefix(home).ok()) {
        Some(rest) => Path::new("~").join(rest).display().to_string(),
        None => game_dir.display().to_string(),
    }
}

fn build_ready_line(build: &str) -> String {
    format!("update {build} ready")
}

fn mod_line(card: &GameCard) -> (String, CardTone) {
    if !card.profile.support.install {
        return ("Not built yet".to_owned(), CardTone::Muted);
    }
    let build = card
        .installed_build
        .as_deref()
        .map(|hash| format!(", build {hash}"))
        .unwrap_or_default();
    match card.mod_status {
        ModStatus::NotInstalled => ("Not installed".to_owned(), CardTone::Warn),
        ModStatus::Current | ModStatus::Outdated => (format!("Installed{build}"), CardTone::Good),
        ModStatus::Foreign => (
            format!(
                "A different {} is in the game folder",
                card.profile.loader_dll_name
            ),
            CardTone::Bad,
        ),
    }
}
