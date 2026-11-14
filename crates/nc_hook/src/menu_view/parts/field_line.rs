use imgui::{StyleVar, Ui};
use nc_ui_style::INK_MUTED;

use crate::menu_view::menu_sizes::{
    MENU_FACT_KEY_WIDTH_PX, MENU_FIELD_HEIGHT_PX, MENU_FIELD_TEXT_TOP_PADDING_PX,
    MENU_LABEL_LEFT_PX, MENU_LINE_HEIGHT_PX,
};
use crate::menu_view::parts::{caps_size, paint_caps};
use crate::menu_view::{MenuFont, ink};

/// An editable text on an underline, keyed like a fact: the key in spaced capitals at the left, no box, the
/// muted rule under the text. Returns true when the text changed.
pub fn field_line(ui: &Ui, key: &str, text: &mut String) -> bool {
    let at = ui.cursor_screen_pos();
    let local = ui.cursor_pos();
    let width = ui.content_region_avail()[0];
    ui.dummy([width, MENU_LINE_HEIGHT_PX]);
    let after = ui.cursor_pos();
    let key_size = caps_size(ui, key, MenuFont::Caps);
    let middle = at[1] + MENU_LINE_HEIGHT_PX / 2.0;
    paint_caps(
        ui,
        &ui.get_window_draw_list(),
        [
            at[0] + MENU_LABEL_LEFT_PX,
            (middle - key_size[1] / 2.0).round(),
        ],
        key,
        MenuFont::Caps,
        INK_MUTED,
    );
    let field_top = (MENU_LINE_HEIGHT_PX - MENU_FIELD_HEIGHT_PX) / 2.0;
    ui.set_cursor_pos([local[0] + MENU_FACT_KEY_WIDTH_PX, local[1] + field_top]);
    let field_width = width - MENU_FACT_KEY_WIDTH_PX;
    let changed = {
        let _border = ui.push_style_var(StyleVar::FrameBorderSize(0.0));
        let _padding = ui.push_style_var(StyleVar::FramePadding([
            0.0,
            MENU_FIELD_TEXT_TOP_PADDING_PX,
        ]));
        ui.set_next_item_width(field_width);
        ui.input_text(format!("##field-{key}"), text).build()
    };
    let rule_y = at[1] + field_top + MENU_FIELD_HEIGHT_PX - 0.5;
    let rule_left = at[0] + MENU_FACT_KEY_WIDTH_PX;
    ui.get_window_draw_list()
        .add_line(
            [rule_left, rule_y],
            [rule_left + field_width, rule_y],
            ink(INK_MUTED),
        )
        .build();
    ui.set_cursor_pos(after);
    changed
}
