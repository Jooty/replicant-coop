use eframe::egui::Ui;

use crate::app::parts::{ControlLook, OptionNesting, option_line, section_heading};
use crate::app::{Launcher, NetForms};

const HEADING_TO_OPTIONS_PX: f32 = 8.0;

pub(super) fn draw_in_game_section(ui: &mut Ui, launcher: &mut Launcher, forms: &mut NetForms) {
    section_heading(ui, "In game", None);
    ui.add_space(HEADING_TO_OPTIONS_PX);
    let mut changed = option_line(
        ui,
        "Keep running in the background",
        &mut launcher.config.keep_running,
        ControlLook::Editable,
        OptionNesting::Top,
    )
    .on_hover_text("The game keeps playing when its window loses focus (always on in dev builds)")
    .changed();
    changed |= option_line(
        ui,
        "Show the event log",
        &mut launcher.config.event_log,
        ControlLook::Editable,
        OptionNesting::Top,
    )
    .on_hover_text("The short messages at the bottom left of the game screen")
    .changed();
    if changed {
        launcher.apply_settings();
        forms.net.set_player_settings(launcher.player_settings());
    }
}
