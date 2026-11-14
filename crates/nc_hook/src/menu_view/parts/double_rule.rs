use imgui::{DrawListMut, Ui};
use nc_ui_style::INK_MUTED;

use crate::menu_view::ink;
use crate::menu_view::menu_sizes::MENU_DOUBLE_RULE_HEIGHT_PX;

/// Two 1 px lines, 4 px apart top to bottom, from `left` to `right`, that close every section.
pub fn paint_double_rule(draw: &DrawListMut<'_>, left: f32, right: f32, top: f32) {
    for y in [top + 0.5, top + MENU_DOUBLE_RULE_HEIGHT_PX - 0.5] {
        draw.add_line([left, y], [right, y], ink(INK_MUTED)).build();
    }
}

pub fn double_rule(ui: &Ui) {
    let at = ui.cursor_screen_pos();
    let width = ui.content_region_avail()[0];
    ui.dummy([width, MENU_DOUBLE_RULE_HEIGHT_PX]);
    paint_double_rule(&ui.get_window_draw_list(), at[0], at[0] + width, at[1]);
}
