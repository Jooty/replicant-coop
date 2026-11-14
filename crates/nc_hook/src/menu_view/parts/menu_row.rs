use imgui::Ui;
use nc_ui_style::{HARD_SHADOW, HATCH_LINE, INK, INK_MUTED, INK_ON_SAND, INK_UNAVAILABLE, SAND};

use crate::menu_view::ink;
use crate::menu_view::menu_sizes::{
    MENU_HARD_SHADOW_OFFSET_PX, MENU_LINE_HEIGHT_PX, MENU_ROW_POINTER_SQUARE_CENTRE_X_PX,
    MENU_ROW_POINTER_SQUARE_SIDE_PX, MENU_ROW_TEXT_LEFT_PX, MENU_ROW_TEXT_RIGHT_PADDING_PX,
};
use crate::menu_view::parts::{paint_hatching, paint_square};

/// One line of a list. The row under the pointer, and the `selected` one, is a sand bar with dark ink, a
/// small square and a hard offset shadow; an unavailable row is hatched and greyed and cannot be picked.
/// `value` is a muted word at the right. Returns true when it was clicked.
pub fn menu_row(
    ui: &Ui,
    label: &str,
    value: Option<&str>,
    selected: bool,
    available: bool,
) -> bool {
    let at = ui.cursor_screen_pos();
    let width = ui.content_region_avail()[0];
    let size = [width, MENU_LINE_HEIGHT_PX];
    let clicked = ui.invisible_button(format!("##row-{label}"), size) && available;
    let inverted = available && (selected || ui.is_item_hovered());
    let max = [at[0] + width, at[1] + MENU_LINE_HEIGHT_PX];
    let middle = at[1] + MENU_LINE_HEIGHT_PX / 2.0;
    let draw = ui.get_window_draw_list();
    if inverted {
        let offset = MENU_HARD_SHADOW_OFFSET_PX;
        draw.add_rect(
            [at[0] + offset, at[1] + offset],
            [max[0] + offset, max[1] + offset],
            ink(HARD_SHADOW),
        )
        .filled(true)
        .build();
        draw.add_rect(at, max, ink(SAND)).filled(true).build();
        paint_square(
            &draw,
            [at[0] + MENU_ROW_POINTER_SQUARE_CENTRE_X_PX, middle],
            MENU_ROW_POINTER_SQUARE_SIDE_PX,
            INK_ON_SAND,
        );
    } else if !available {
        paint_hatching(&draw, at, max, HATCH_LINE);
    }
    let (label_ink, value_ink) = match (available, inverted) {
        (false, _) => (INK_UNAVAILABLE, INK_UNAVAILABLE),
        (true, true) => (INK_ON_SAND, INK_ON_SAND),
        (true, false) => (INK, INK_MUTED),
    };
    let text_top = (middle - ui.text_line_height() / 2.0).round();
    draw.with_clip_rect_intersect(
        at,
        [max[0] - MENU_ROW_TEXT_RIGHT_PADDING_PX, max[1]],
        || {
            draw.add_text(
                [at[0] + MENU_ROW_TEXT_LEFT_PX, text_top],
                ink(label_ink),
                label,
            );
        },
    );
    if let Some(value) = value {
        let value_width = ui.calc_text_size(value)[0];
        draw.add_text(
            [
                max[0] - MENU_ROW_TEXT_RIGHT_PADDING_PX - value_width,
                text_top,
            ],
            ink(value_ink),
            value,
        );
    }
    clicked
}
