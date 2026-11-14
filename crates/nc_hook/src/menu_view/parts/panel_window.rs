use imgui::{Condition, Ui, WindowFlags};
use nc_ui_style::{INK, INK_MUTED};

use crate::menu_view::MenuFont;
use crate::menu_view::menu_sizes::{
    MENU_DOUBLE_RULE_HEIGHT_PX, MENU_HEADING_ROW_HEIGHT_PX, MENU_HEADING_SQUARE_SIDE_PX,
    MENU_HEADING_SQUARE_TO_TEXT_PX, MENU_HEADING_TO_BODY_GAP_PX, MENU_HEADING_TO_RULE_GAP_PX,
};
use crate::menu_view::parts::{
    caps_size, paint_caps, paint_double_rule, paint_square, paint_window_shadow,
};

const CLOSE_WORD: &str = "Close";

/// A panel the menu opens: a window with no title bar whose first line is its heading, the square and the
/// title in spaced capitals with CLOSE at the right, then the doubled rule. Clicking CLOSE clears `open`.
pub fn panel_window(
    ui: &Ui,
    title: &str,
    open: &mut bool,
    position: [f32; 2],
    size: [f32; 2],
    body: impl FnOnce(),
) {
    if !*open {
        return;
    }
    ui.window(format!("{title}##panel"))
        .position(position, Condition::FirstUseEver)
        .size(size, Condition::FirstUseEver)
        .flags(WindowFlags::NO_TITLE_BAR | WindowFlags::NO_COLLAPSE)
        .build(|| {
            paint_window_shadow(ui);
            if panel_heading(ui, title) {
                *open = false;
            }
            body();
        });
}

fn panel_heading(ui: &Ui, title: &str) -> bool {
    let at = ui.cursor_screen_pos();
    let local = ui.cursor_pos();
    let width = ui.content_region_avail()[0];
    let block =
        MENU_HEADING_ROW_HEIGHT_PX + MENU_HEADING_TO_RULE_GAP_PX + MENU_DOUBLE_RULE_HEIGHT_PX;
    ui.dummy([width, block + MENU_HEADING_TO_BODY_GAP_PX]);
    let after = ui.cursor_pos();
    let close_size = caps_size(ui, CLOSE_WORD, MenuFont::Caps);
    ui.set_cursor_pos([local[0] + width - close_size[0], local[1]]);
    let closed = ui.invisible_button("##panel-close", [close_size[0], MENU_HEADING_ROW_HEIGHT_PX]);
    let close_ink = if ui.is_item_hovered() { INK } else { INK_MUTED };
    ui.set_cursor_pos(after);
    let middle = at[1] + MENU_HEADING_ROW_HEIGHT_PX / 2.0;
    let draw = ui.get_window_draw_list();
    paint_square(
        &draw,
        [at[0] + MENU_HEADING_SQUARE_SIDE_PX / 2.0, middle],
        MENU_HEADING_SQUARE_SIDE_PX,
        INK,
    );
    let title_size = caps_size(ui, title, MenuFont::Heading);
    paint_caps(
        ui,
        &draw,
        [
            at[0] + MENU_HEADING_SQUARE_SIDE_PX + MENU_HEADING_SQUARE_TO_TEXT_PX,
            (middle - title_size[1] / 2.0).round(),
        ],
        title,
        MenuFont::Heading,
        INK,
    );
    paint_caps(
        ui,
        &draw,
        [
            at[0] + width - close_size[0],
            (middle - close_size[1] / 2.0).round(),
        ],
        CLOSE_WORD,
        MenuFont::Caps,
        close_ink,
    );
    let rule_top = at[1] + MENU_HEADING_ROW_HEIGHT_PX + MENU_HEADING_TO_RULE_GAP_PX;
    paint_double_rule(&draw, at[0], at[0] + width, rule_top);
    closed
}
