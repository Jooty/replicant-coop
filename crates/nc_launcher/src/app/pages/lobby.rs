use eframe::egui::Ui;

use crate::app::{Launcher, NetForms};
use crate::net::{LOBBY_MODE_CONTINUE, NetState};

pub(super) fn session_open(forms: &NetForms) -> bool {
    !forms.closing_session()
        && !matches!(
            forms.net.status().state,
            NetState::Idle | NetState::Failed(_)
        )
}

pub(super) fn claim_mode(launcher: &mut Launcher, forms: &NetForms, mode: u8) -> bool {
    if launcher.lobby_mode == mode {
        return true;
    }
    // The host picks the kind of game inside the lobby until it starts; a guest's follows the host's.
    if !session_open(forms) || (!launcher.link.guest && !launcher.lobby_started) {
        launcher.lobby_mode = mode;
        return true;
    }
    false
}

pub(super) fn other_page_note(ui: &mut Ui, launcher: &Launcher) {
    ui.label(format!(
        "A lobby is open on the {} page.",
        ui_mode_name(launcher.lobby_mode)
    ));
}

fn ui_mode_name(mode: u8) -> &'static str {
    if mode == LOBBY_MODE_CONTINUE {
        "Continue"
    } else {
        "New game"
    }
}
