use imgui::Ui;
use nc_ui_style::{HOVER_WASH, INK, INK_MUTED};

use crate::menu_view::menu_sizes::{
    MENU_DOUBLE_RULE_HEIGHT_PX, MENU_HEADING_ROW_HEIGHT_PX, MENU_HEADING_SQUARE_SIDE_PX,
    MENU_HEADING_SQUARE_TO_TEXT_PX, MENU_HEADING_TO_RULE_GAP_PX,
};
use crate::menu_view::parts::{
    caps_size, paint_caps, paint_double_rule, paint_open_square, paint_square,
};
use crate::menu_view::{MenuFont, ink};

const HEADING_BLOCK_HEIGHT_PX: f32 =
    MENU_HEADING_ROW_HEIGHT_PX + MENU_HEADING_TO_RULE_GAP_PX + MENU_DOUBLE_RULE_HEIGHT_PX;

/// A section's heading: a small solid square, the title in spaced medium capitals, an optional muted note at the
/// right, then the doubled rule.
pub fn section_heading(ui: &Ui, title: &str, right_note: Option<&str>) {
    let at = ui.cursor_screen_pos();
    let width = ui.content_region_avail()[0];
    ui.dummy([width, HEADING_BLOCK_HEIGHT_PX]);
    paint_heading(ui, at, width, title, right_note, true, false);
}

/// A heading that folds its section: clicking it flips `open`. The square is solid while the section is
/// open and an outline while it is folded. Returns true when it was clicked.
pub fn fold_heading(ui: &Ui, title: &str, open: &mut bool) -> bool {
    let at = ui.cursor_screen_pos();
    let width = ui.content_region_avail()[0];
    let clicked = ui.invisible_button(format!("##fold-{title}"), [width, HEADING_BLOCK_HEIGHT_PX]);
    if clicked {
        *open = !*open;
    }
    let hovered = ui.is_item_hovered();
    paint_heading(ui, at, width, title, None, *open, hovered);
    clicked
}

fn paint_heading(
    ui: &Ui,
    at: [f32; 2],
    width: f32,
    title: &str,
    right_note: Option<&str>,
    solid: bool,
    hovered: bool,
) {
    let draw = ui.get_window_draw_list();
    let middle = at[1] + MENU_HEADING_ROW_HEIGHT_PX / 2.0;
    if hovered {
        draw.add_rect(
            at,
            [at[0] + width, at[1] + MENU_HEADING_ROW_HEIGHT_PX],
            ink(HOVER_WASH),
        )
        .filled(true)
        .build();
    }
    let square_centre = [at[0] + MENU_HEADING_SQUARE_SIDE_PX / 2.0, middle];
    if solid {
        paint_square(&draw, square_centre, MENU_HEADING_SQUARE_SIDE_PX, INK);
    } else {
        paint_open_square(&draw, square_centre, MENU_HEADING_SQUARE_SIDE_PX, INK);
    }
    let title_size = caps_size(ui, title, MenuFont::Heading);
    let text_left = at[0] + MENU_HEADING_SQUARE_SIDE_PX + MENU_HEADING_SQUARE_TO_TEXT_PX;
    paint_caps(
        ui,
        &draw,
        [text_left, (middle - title_size[1] / 2.0).round()],
        title,
        MenuFont::Heading,
        INK,
    );
    if let Some(note) = right_note {
        let size = caps_size(ui, note, MenuFont::Caps);
        paint_caps(
            ui,
            &draw,
            [at[0] + width - size[0], (middle - size[1] / 2.0).round()],
            note,
            MenuFont::Caps,
            INK_MUTED,
        );
    }
    let rule_top = at[1] + MENU_HEADING_ROW_HEIGHT_PX + MENU_HEADING_TO_RULE_GAP_PX;
    paint_double_rule(&draw, at[0], at[0] + width, rule_top);
}
