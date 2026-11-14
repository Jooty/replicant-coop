use eframe::egui::Ui;
use nc_launcher_core::{current_mode, lobby_permissions, rules_for_mode};

use crate::app::parts::{MenuValue, menu_row, section_heading};
use crate::app::{Launcher, NetForms};

const MODE_HEADING: &str = "Mode";
const HEADING_TO_MODES_GAP_PX: f32 = 10.0;

pub(super) fn draw_mode_menu(ui: &mut Ui, launcher: &mut Launcher, forms: &NetForms) {
    let profile = launcher.profile;
    let may_change = lobby_permissions(launcher.link.guest).change_mode;
    let current = current_mode(profile, launcher.link.rules);
    section_heading(ui, MODE_HEADING, None);
    ui.add_space(HEADING_TO_MODES_GAP_PX);
    for mode in profile.modes {
        let selected = current.is_some_and(|current| current.id == mode.id);
        let row = menu_row(ui, mode.label, MenuValue::None, selected, true);
        if row.clicked()
            && may_change
            && !selected
            && let Some(rules) = rules_for_mode(profile, mode.id)
        {
            forms.net.set_rules(rules);
            launcher.link.rules = rules;
        }
    }
}
