use imgui::Ui;
use nc_ui_style::{INK_MUTED, rgba_to_f32};

use crate::menu_view::MenuFont;
use crate::menu_view::menu_sizes::{
    MENU_FACT_KEY_WIDTH_PX, MENU_LABEL_LEFT_PX, MENU_LINE_HEIGHT_PX,
};
use crate::menu_view::parts::{caps_size, paint_caps};

/// One fact: a spaced-capital key in a fixed column at the left and the value beside it in `value_ink`,
/// wrapped to the width left. The key sits at the top, so a value of two lines reads from its first line.
pub fn fact_line(ui: &Ui, key: &str, value: &str, value_ink: [u8; 4]) {
    let at = ui.cursor_screen_pos();
    let local = ui.cursor_pos();
    let width = ui.content_region_avail()[0];
    ui.dummy([width, MENU_LINE_HEIGHT_PX]);
    let after_row = ui.cursor_pos();
    let line_height = ui.text_line_height();
    let key_height = caps_size(ui, key, MenuFont::Caps)[1];
    paint_caps(
        ui,
        &ui.get_window_draw_list(),
        [
            at[0] + MENU_LABEL_LEFT_PX,
            (at[1] + (MENU_LINE_HEIGHT_PX - key_height) / 2.0).round(),
        ],
        key,
        MenuFont::Caps,
        INK_MUTED,
    );
    ui.set_cursor_pos([
        local[0] + MENU_FACT_KEY_WIDTH_PX,
        (local[1] + (MENU_LINE_HEIGHT_PX - line_height) / 2.0).round(),
    ]);
    let wrap = ui.push_text_wrap_pos_with_pos(local[0] + width);
    ui.text_colored(rgba_to_f32(value_ink), value);
    drop(wrap);
    if ui.cursor_pos()[1] < after_row[1] {
        ui.set_cursor_pos(after_row);
    }
}
