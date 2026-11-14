use eframe::egui::{RichText, Ui};

use crate::app::pages::campaign_lobby::{LobbyMenu, draw_campaign_lobby};
use crate::app::pages::lobby::{claim_mode, other_page_note, session_open};
use crate::app::palette::INK_MUTED;
use crate::app::style::body_font;
use crate::app::{Launcher, NetForms};
use crate::net::LOBBY_MODE_CONTINUE;

const NO_SAVES_NOTE: &str =
    "This game has no co-op saves in the launcher yet; start a campaign from New game.";

pub(super) fn draw_continue(ui: &mut Ui, launcher: &mut Launcher, forms: &mut NetForms) {
    if !launcher.profile.support.saves {
        ui.label(
            RichText::new(NO_SAVES_NOTE)
                .font(body_font())
                .color(INK_MUTED),
        );
        return;
    }
    if !claim_mode(launcher, forms, LOBBY_MODE_CONTINUE) {
        other_page_note(ui, launcher);
        return;
    }
    if !session_open(forms) {
        return;
    }
    let menu = if launcher.link.guest {
        LobbyMenu::Campaign
    } else {
        LobbyMenu::Saves
    };
    draw_campaign_lobby(ui, launcher, forms, menu);
}
