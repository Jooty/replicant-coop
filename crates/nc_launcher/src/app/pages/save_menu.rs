use eframe::egui::Ui;

use crate::app::Launcher;
use crate::app::parts::{MenuValue, menu_row, section_heading};
use crate::saves::slot_label;

const SAVES_HEADING: &str = "Co-op saves";
const SAVES_NOTE: &str = "Level, time";
const HEADING_TO_SAVES_GAP_PX: f32 = 10.0;

pub(super) fn draw_save_menu(ui: &mut Ui, launcher: &mut Launcher) {
    section_heading(ui, SAVES_HEADING, Some(SAVES_NOTE));
    ui.add_space(HEADING_TO_SAVES_GAP_PX);
    let mut picked = launcher.selected_slot;
    for slot in &launcher.slots {
        let name = if slot.used { slot.name.trim() } else { "Empty" };
        let label = format!("{}  {name}", slot_label(slot.index));
        let figures = format!("Lv {}  {}", slot.level, slot.play_time_short());
        let value = if slot.used {
            MenuValue::Plain(&figures)
        } else {
            MenuValue::None
        };
        let selected = slot.used && picked == Some(slot.index);
        if menu_row(ui, &label, value, selected, slot.used).clicked() {
            picked = Some(slot.index);
        }
    }
    launcher.selected_slot = picked;
}
