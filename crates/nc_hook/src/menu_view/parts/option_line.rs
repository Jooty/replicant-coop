use imgui::{DrawListMut, Ui};
use nc_ui_style::{ControlLook, HAIRLINE_STRONG, HATCH_LINE, OptionNesting, rgba_to_f32};

use crate::menu_view::menu_sizes::{
    MENU_BRANCH_LABEL_LEFT_PX, MENU_LABEL_LEFT_PX, MENU_LABEL_TO_SWITCH_GAP_PX,
    MENU_LINE_HEIGHT_PX, MENU_ON_OFF_SQUARE_SIDE_PX, MENU_ON_OFF_TEXT_TO_SQUARE_PX,
    MENU_TREE_ARM_END_X_PX, MENU_TREE_X_PX,
};
use crate::menu_view::parts::{
    caps_size, paint_caps, paint_hatching, paint_open_square, paint_square,
};
use crate::menu_view::{MenuFont, ink};

/// One on/off option: its label, a tree line when it is nested, and "ON" with a filled square or "OFF" with an
/// open one at the right. An unavailable option is hatched and greyed, a read-only one muted; clicking
/// anywhere on an editable line flips `on`. Returns true when it flipped.
pub fn option_line(
    ui: &Ui,
    label: &str,
    on: &mut bool,
    look: ControlLook,
    nesting: OptionNesting,
) -> bool {
    let at = ui.cursor_screen_pos();
    let local = ui.cursor_pos();
    let width = ui.content_region_avail()[0];
    let line_height = ui.text_line_height();
    let label_left = label_left_px(nesting);
    let label_width = width - label_left - on_off_width(ui) - MENU_LABEL_TO_SWITCH_GAP_PX;
    let label_height = ui.calc_text_size_with_opts(label, false, label_width)[1];
    let height = MENU_LINE_HEIGHT_PX.max(label_height + MENU_LINE_HEIGHT_PX - line_height);
    let size = [width, height];
    let flipped = if look.editable() {
        ui.invisible_button(format!("##option-{label}"), size)
    } else {
        ui.dummy(size);
        false
    };
    if flipped {
        *on = !*on;
    }
    let after = ui.cursor_pos();
    let max = [at[0] + width, at[1] + height];
    let first_line_middle = at[1] + MENU_LINE_HEIGHT_PX / 2.0;
    {
        let draw = ui.get_window_draw_list();
        if look == ControlLook::Unavailable {
            paint_hatching(&draw, at, max, HATCH_LINE);
        }
        paint_tree_lines(&draw, at, height, nesting);
        paint_on_off(ui, &draw, [max[0], first_line_middle], *on, look);
    }
    ui.set_cursor_pos([
        local[0] + label_left,
        (local[1] + (MENU_LINE_HEIGHT_PX - line_height) / 2.0).round(),
    ]);
    let wrap = ui.push_text_wrap_pos_with_pos(local[0] + label_left + label_width);
    ui.text_colored(rgba_to_f32(look.label_ink()), label);
    drop(wrap);
    ui.set_cursor_pos(after);
    flipped
}

/// An editable top-level option, the menu's stand-in for a checkbox. Returns true when it flipped.
pub fn switch_line(ui: &Ui, label: &str, on: &mut bool) -> bool {
    option_line(ui, label, on, ControlLook::Editable, OptionNesting::Top)
}

fn label_left_px(nesting: OptionNesting) -> f32 {
    if nesting.is_nested() {
        MENU_BRANCH_LABEL_LEFT_PX
    } else {
        MENU_LABEL_LEFT_PX
    }
}

fn on_off_width(ui: &Ui) -> f32 {
    let word = caps_size(ui, "Off", MenuFont::Caps)[0].max(caps_size(ui, "On", MenuFont::Caps)[0]);
    word + MENU_ON_OFF_TEXT_TO_SQUARE_PX + MENU_ON_OFF_SQUARE_SIDE_PX
}

fn paint_tree_lines(draw: &DrawListMut<'_>, at: [f32; 2], height: f32, nesting: OptionNesting) {
    let bottom = match nesting {
        OptionNesting::Top => return,
        OptionNesting::Branch => at[1] + height,
        OptionNesting::LastBranch => at[1] + MENU_LINE_HEIGHT_PX / 2.0,
    };
    let x = at[0] + MENU_TREE_X_PX + 0.5;
    let arm_y = at[1] + MENU_LINE_HEIGHT_PX / 2.0 + 0.5;
    let color = ink(HAIRLINE_STRONG);
    draw.add_line([x, at[1]], [x, bottom], color).build();
    draw.add_line([x, arm_y], [at[0] + MENU_TREE_ARM_END_X_PX, arm_y], color)
        .build();
}

fn paint_on_off(
    ui: &Ui,
    draw: &DrawListMut<'_>,
    right_middle: [f32; 2],
    on: bool,
    look: ControlLook,
) {
    let color = look.switch_ink(on);
    let word = if on { "On" } else { "Off" };
    let size = caps_size(ui, word, MenuFont::Caps);
    let square_centre = [
        right_middle[0] - MENU_ON_OFF_SQUARE_SIDE_PX / 2.0,
        right_middle[1],
    ];
    let word_left =
        right_middle[0] - MENU_ON_OFF_SQUARE_SIDE_PX - MENU_ON_OFF_TEXT_TO_SQUARE_PX - size[0];
    paint_caps(
        ui,
        draw,
        [word_left, (right_middle[1] - size[1] / 2.0).round()],
        word,
        MenuFont::Caps,
        color,
    );
    if on {
        paint_square(draw, square_centre, MENU_ON_OFF_SQUARE_SIDE_PX, color);
    } else {
        paint_open_square(draw, square_centre, MENU_ON_OFF_SQUARE_SIDE_PX, color);
    }
}
