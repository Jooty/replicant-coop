use std::path::{Path, PathBuf};

use eframe::egui::Pos2;
use nc_launcher_core::{
    CoopSaveDeleteOffer, DeletedCoopSaves, GameProfile, coop_save_delete_offer, coop_saves_in,
    delete_coop_saves,
};

use crate::app::Launcher;
use crate::install::game_running;
use crate::saves::{BACKUP_FOLDER, backup_root, format_stamp, save_dir, save_folders};

/// The right-click menu on a home row: which game it is for, where it opened, how many co-op saves that
/// game had when it opened, and whether the delete waits for its second click.
#[derive(Debug, Clone, PartialEq)]
pub struct CoopSaveMenu {
    pub profile: &'static GameProfile,
    pub anchor: Pos2,
    pub coop_saves: usize,
    pub confirming: bool,
}

impl Launcher {
    pub fn open_coop_save_menu(&mut self, profile: &'static GameProfile, anchor: Pos2) {
        let coop_saves = self
            .coop_saves_of(profile)
            .map_or(0, |(_, saves)| saves.len());
        self.coop_save_menu = Some(CoopSaveMenu {
            profile,
            anchor,
            coop_saves,
            confirming: false,
        });
    }

    /// What the open menu offers now; `lobby_open` is whether a lobby of the menu's game is open.
    pub fn coop_save_menu_offer(&self, lobby_open: bool) -> Option<CoopSaveDeleteOffer> {
        let menu = self.coop_save_menu.as_ref()?;
        let running = self.any_game_running && menu.profile.game_id == self.profile.game_id;
        Some(coop_save_delete_offer(menu.coop_saves, running, lobby_open))
    }

    /// Deletes the game's co-op saves if that is allowed now, checking again what is there and whether the
    /// game runs, and logs what happened.
    pub fn delete_coop_saves_of(&mut self, profile: &'static GameProfile, lobby_open: bool) {
        self.coop_save_menu = None;
        let (save_dir, saves) = self.coop_saves_of(profile).unwrap_or_default();
        let offer = coop_save_delete_offer(saves.len(), game_running(profile), lobby_open);
        if let Some(refusal) = offer.refusal_line() {
            self.log.push(format!("co-op saves not deleted: {refusal}"));
            return;
        }
        let Some(root) = backup_root() else {
            self.log
                .push("co-op saves not deleted: the launcher has no data folder".to_owned());
            return;
        };
        let stamp = format_stamp(&chrono::Utc::now());
        match delete_coop_saves(&save_dir, &saves, &root, &stamp) {
            Ok(deleted) => self.log.push(deleted_line(profile, &deleted)),
            Err(error) => self.log.push(format!("co-op saves not deleted: {error}")),
        }
        self.refresh();
    }

    fn coop_saves_of(&self, profile: &GameProfile) -> Option<(PathBuf, Vec<PathBuf>)> {
        let install = self
            .cards
            .iter()
            .find(|card| card.profile.game_id == profile.game_id)?
            .install
            .as_ref()?;
        let dir = save_dir(install)?;
        let saves = coop_saves_in(&save_folders(&dir));
        Some((dir, saves))
    }
}

fn deleted_line(profile: &GameProfile, deleted: &DeletedCoopSaves) -> String {
    let folder = deleted
        .folder
        .file_name()
        .map(Path::new)
        .unwrap_or(&deleted.folder);
    format!(
        "deleted the co-op saves of {}: {} files, moved to {BACKUP_FOLDER}/{} in the launcher's data folder",
        profile.display_name,
        deleted.files,
        folder.display()
    )
}
