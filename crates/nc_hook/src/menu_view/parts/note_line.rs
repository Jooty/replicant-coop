use imgui::Ui;
use nc_ui_style::rgba_to_f32;

use crate::menu_view::menu_sizes::{MENU_LABEL_LEFT_PX, MENU_NOTE_LINE_GAP_PX};

/// A sentence the game reports, in `color`, wrapped to the width and set in from the left like a label.
pub fn note_line(ui: &Ui, text: &str, color: [u8; 4]) {
    let local = ui.cursor_pos();
    let width = ui.content_region_avail()[0];
    ui.set_cursor_pos([
        local[0] + MENU_LABEL_LEFT_PX,
        local[1] + MENU_NOTE_LINE_GAP_PX,
    ]);
    let wrap = ui.push_text_wrap_pos_with_pos(local[0] + width);
    ui.text_colored(rgba_to_f32(color), text);
    drop(wrap);
    let after = ui.cursor_pos();
    ui.set_cursor_pos([local[0], after[1] + MENU_NOTE_LINE_GAP_PX]);
    ui.dummy([0.0, 0.0]);
}
