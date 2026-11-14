use imgui::Ui;
use nc_ui_style::INK_MUTED;

use crate::menu_view::MenuFont;
use crate::menu_view::menu_sizes::{MENU_LABEL_LEFT_PX, MENU_SUB_HEADING_HEIGHT_PX};
use crate::menu_view::parts::{caps_size, paint_caps};

/// A muted line of spaced capitals that names a group inside a section.
pub fn sub_heading(ui: &Ui, text: &str) {
    let at = ui.cursor_screen_pos();
    let width = ui.content_region_avail()[0];
    ui.dummy([width, MENU_SUB_HEADING_HEIGHT_PX]);
    let size = caps_size(ui, text, MenuFont::Caps);
    let top = (at[1] + (MENU_SUB_HEADING_HEIGHT_PX - size[1]) / 2.0).round();
    paint_caps(
        ui,
        &ui.get_window_draw_list(),
        [at[0] + MENU_LABEL_LEFT_PX, top],
        text,
        MenuFont::Caps,
        INK_MUTED,
    );
}
