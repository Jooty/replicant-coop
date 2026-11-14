use imgui::Ui;
use nc_ui_style::{
    HARD_SHADOW, HATCH_LINE, HOVER_WASH, INK, INK_MUTED, INK_ON_SAND, INK_UNAVAILABLE, SAND,
};

use crate::menu_view::menu_sizes::{
    MENU_HARD_SHADOW_OFFSET_PX, MENU_LINE_BUTTON_GAP_PX, MENU_LINE_BUTTON_HEIGHT_PX,
    MENU_LINE_BUTTON_SIDE_PADDING_PX, MENU_LINE_BUTTONS_MARGIN_PX,
};
use crate::menu_view::parts::{caps_size, paint_caps, paint_hatching};
use crate::menu_view::{MenuFont, ink};

/// An outlined button with a spaced-capital label. Hovering washes it and brings the border to full ink; a
/// `held` one (a panel that is open) is a sand block with dark ink and a hard shadow; a disabled one is
/// hatched and greyed. Returns true when it was clicked.
pub fn line_button(ui: &Ui, label: &str, held: bool, enabled: bool) -> bool {
    let at = ui.cursor_screen_pos();
    let width = line_button_width(ui, label);
    let clicked = ui.invisible_button(
        format!("##button-{label}"),
        [width, MENU_LINE_BUTTON_HEIGHT_PX],
    ) && enabled;
    let hovered = ui.is_item_hovered();
    let max = [at[0] + width, at[1] + MENU_LINE_BUTTON_HEIGHT_PX];
    let draw = ui.get_window_draw_list();
    let label_ink = if !enabled {
        paint_hatching(&draw, at, max, HATCH_LINE);
        draw.add_rect(
            [at[0] + 0.5, at[1] + 0.5],
            [max[0] - 0.5, max[1] - 0.5],
            ink(INK_UNAVAILABLE),
        )
        .build();
        INK_UNAVAILABLE
    } else if held {
        let offset = MENU_HARD_SHADOW_OFFSET_PX;
        draw.add_rect(
            [at[0] + offset, at[1] + offset],
            [max[0] + offset, max[1] + offset],
            ink(HARD_SHADOW),
        )
        .filled(true)
        .build();
        draw.add_rect(at, max, ink(SAND)).filled(true).build();
        INK_ON_SAND
    } else {
        if hovered {
            draw.add_rect(at, max, ink(HOVER_WASH)).filled(true).build();
        }
        let border = if hovered { INK } else { INK_MUTED };
        draw.add_rect(
            [at[0] + 0.5, at[1] + 0.5],
            [max[0] - 0.5, max[1] - 0.5],
            ink(border),
        )
        .build();
        INK
    };
    let size = caps_size(ui, label, MenuFont::Caps);
    paint_caps(
        ui,
        &draw,
        [
            (at[0] + (width - size[0]) / 2.0).round(),
            (at[1] + (MENU_LINE_BUTTON_HEIGHT_PX - size[1]) / 2.0).round(),
        ],
        label,
        MenuFont::Caps,
        label_ink,
    );
    clicked
}

/// A run of line buttons that wraps onto a new line when the next one does not fit. `held` marks the ones
/// drawn as held. Returns the index of the button clicked, if any.
pub fn line_buttons(ui: &Ui, labels: &[&str], held: &[bool]) -> Option<usize> {
    let available = ui.content_region_avail()[0];
    ui.dummy([0.0, MENU_LINE_BUTTONS_MARGIN_PX]);
    let mut used = 0.0;
    let mut clicked = None;
    for (index, label) in labels.iter().enumerate() {
        let width = line_button_width(ui, label);
        if index > 0 {
            if used + MENU_LINE_BUTTON_GAP_PX + width <= available {
                ui.same_line_with_spacing(0.0, MENU_LINE_BUTTON_GAP_PX);
                used += MENU_LINE_BUTTON_GAP_PX;
            } else {
                ui.dummy([
                    0.0,
                    MENU_LINE_BUTTON_GAP_PX - ui.clone_style().item_spacing[1],
                ]);
                used = 0.0;
            }
        }
        if line_button(ui, label, held.get(index).copied().unwrap_or(false), true) {
            clicked = Some(index);
        }
        used += width;
    }
    ui.dummy([0.0, MENU_LINE_BUTTONS_MARGIN_PX]);
    clicked
}

fn line_button_width(ui: &Ui, label: &str) -> f32 {
    (caps_size(ui, label, MenuFont::Caps)[0] + 2.0 * MENU_LINE_BUTTON_SIDE_PADDING_PX).round()
}
