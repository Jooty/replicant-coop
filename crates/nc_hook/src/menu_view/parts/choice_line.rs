use imgui::Ui;
use nc_ui_style::{INK, INK_MUTED};

use crate::menu_view::menu_sizes::{
    MENU_CHOICE_GAP_PX, MENU_LABEL_LEFT_PX, MENU_LINE_HEIGHT_PX, MENU_ON_OFF_SQUARE_SIDE_PX,
    MENU_ON_OFF_TEXT_TO_SQUARE_PX,
};
use crate::menu_view::parts::{caps_size, paint_caps, paint_open_square, paint_square};
use crate::menu_view::{MenuFont, ink};

/// A label and a row of choices at the right, each a word in spaced capitals with a square: solid for the one
/// in force, an outline for the rest. Returns the choice clicked, if any.
pub fn choice_line(
    ui: &Ui,
    label: &str,
    choices: &[&str],
    current: Option<usize>,
) -> Option<usize> {
    let at = ui.cursor_screen_pos();
    let local = ui.cursor_pos();
    let width = ui.content_region_avail()[0];
    ui.dummy([width, MENU_LINE_HEIGHT_PX]);
    let after = ui.cursor_pos();
    let middle = at[1] + MENU_LINE_HEIGHT_PX / 2.0;
    let widths: Vec<f32> = choices
        .iter()
        .map(|choice| {
            caps_size(ui, choice, MenuFont::Caps)[0]
                + MENU_ON_OFF_TEXT_TO_SQUARE_PX
                + MENU_ON_OFF_SQUARE_SIDE_PX
        })
        .collect();
    let total =
        widths.iter().sum::<f32>() + MENU_CHOICE_GAP_PX * widths.len().saturating_sub(1) as f32;
    let mut left = width - total;
    let mut picked = None;
    for (index, (choice, choice_width)) in choices.iter().zip(&widths).enumerate() {
        ui.set_cursor_pos([local[0] + left, local[1]]);
        if ui.invisible_button(
            format!("##choice-{label}-{index}"),
            [*choice_width, MENU_LINE_HEIGHT_PX],
        ) {
            picked = Some(index);
        }
        let chosen = current == Some(index);
        let color = if chosen { INK } else { INK_MUTED };
        let draw = ui.get_window_draw_list();
        let size = caps_size(ui, choice, MenuFont::Caps);
        let word_left = at[0] + left;
        paint_caps(
            ui,
            &draw,
            [word_left, (middle - size[1] / 2.0).round()],
            choice,
            MenuFont::Caps,
            color,
        );
        let square_centre = [
            at[0] + left + choice_width - MENU_ON_OFF_SQUARE_SIDE_PX / 2.0,
            middle,
        ];
        if chosen {
            paint_square(&draw, square_centre, MENU_ON_OFF_SQUARE_SIDE_PX, color);
        } else {
            paint_open_square(&draw, square_centre, MENU_ON_OFF_SQUARE_SIDE_PX, color);
        }
        left += choice_width + MENU_CHOICE_GAP_PX;
    }
    ui.set_cursor_pos(after);
    ui.get_window_draw_list().add_text(
        [
            at[0] + MENU_LABEL_LEFT_PX,
            (middle - ui.text_line_height() / 2.0).round(),
        ],
        ink(INK),
        label,
    );
    picked
}
