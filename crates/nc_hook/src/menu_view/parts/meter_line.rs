use imgui::{DrawListMut, MouseButton, Ui};
use nc_ui_style::{ControlLook, INK_UNAVAILABLE};

use crate::menu_view::ink;
use crate::menu_view::menu_sizes::{
    MENU_LABEL_LEFT_PX, MENU_LINE_HEIGHT_PX, MENU_METER_CELL_GAP_PX, MENU_METER_CELL_HEIGHT_PX,
    MENU_METER_CELL_WIDTH_PX, MENU_METER_LINE_HEIGHT_PX, MENU_METER_ROW_HEIGHT_PX,
};

/// How a meter turns the pointer into an amount: to the cell under it, as the launcher's meters do, or to
/// the exact share of the meter's width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeterStep {
    Cells,
    Exact,
}

/// A labelled amount from `min` to `max`: the label on one line, a row of `cells` boxes standing in for a
/// slider under it, and the amount followed by `unit` at the right. Clicking or dragging over the boxes sets
/// the amount when the look is editable. Returns true when it changed.
pub fn meter_line(
    ui: &Ui,
    label: &str,
    value: &mut i32,
    range: (i32, i32),
    cells: u32,
    step: MeterStep,
    unit: &str,
    look: ControlLook,
) -> bool {
    let (min, max) = (range.0, range.1.max(range.0 + 1));
    let cells = cells.max(1);
    let at = ui.cursor_screen_pos();
    let local = ui.cursor_pos();
    let width = ui.content_region_avail()[0];
    ui.dummy([width, MENU_METER_LINE_HEIGHT_PX]);
    let after = ui.cursor_pos();
    let meter_left = at[0] + MENU_LABEL_LEFT_PX;
    let meter_top = at[1] + MENU_LINE_HEIGHT_PX;
    let meter_width =
        cells as f32 * MENU_METER_CELL_WIDTH_PX + (cells - 1) as f32 * MENU_METER_CELL_GAP_PX;
    let mut changed = false;
    if look.editable() {
        ui.set_cursor_pos([
            local[0] + MENU_LABEL_LEFT_PX,
            local[1] + MENU_LINE_HEIGHT_PX,
        ]);
        ui.invisible_button(
            format!("##meter-{label}"),
            [meter_width, MENU_METER_ROW_HEIGHT_PX],
        );
        if ui.is_item_active() && ui.is_mouse_down(MouseButton::Left) {
            let share = ((ui.io().mouse_pos[0] - meter_left) / meter_width).clamp(0.0, 1.0);
            let picked = pick(share, min, max, cells, step);
            if picked != *value {
                *value = picked;
                changed = true;
            }
        }
        ui.set_cursor_pos(after);
    }
    let draw = ui.get_window_draw_list();
    let text_top = |middle: f32| (middle - ui.text_line_height() / 2.0).round();
    draw.add_text(
        [meter_left, text_top(at[1] + MENU_LINE_HEIGHT_PX / 2.0)],
        ink(look.label_ink()),
        label,
    );
    let meter_middle = meter_top + MENU_METER_ROW_HEIGHT_PX / 2.0;
    let amount = format!("{value}{unit}");
    let amount_width = ui.calc_text_size(&amount)[0];
    draw.add_text(
        [at[0] + width - amount_width, text_top(meter_middle)],
        ink(look.amount_ink()),
        &amount,
    );
    let span = i64::from(max - min);
    let above_min = i64::from((*value).clamp(min, max) - min);
    let filled = u32::try_from((above_min * i64::from(cells) + span - 1) / span).unwrap_or(0);
    paint_cells(
        &draw,
        [meter_left, meter_middle],
        cells,
        filled,
        look.amount_ink(),
    );
    changed
}

fn pick(share: f32, min: i32, max: i32, cells: u32, step: MeterStep) -> i32 {
    let span = (max - min) as f32;
    match step {
        MeterStep::Exact => min + (share * span).round() as i32,
        MeterStep::Cells => {
            let cell = (share * cells as f32).ceil();
            min + (cell * span / cells as f32).round() as i32
        }
    }
}

fn paint_cells(
    draw: &DrawListMut<'_>,
    left_middle: [f32; 2],
    cells: u32,
    filled: u32,
    fill: [u8; 4],
) {
    let top = (left_middle[1] - MENU_METER_CELL_HEIGHT_PX / 2.0).round();
    for index in 0..cells {
        let left =
            left_middle[0] + index as f32 * (MENU_METER_CELL_WIDTH_PX + MENU_METER_CELL_GAP_PX);
        let right = left + MENU_METER_CELL_WIDTH_PX;
        let bottom = top + MENU_METER_CELL_HEIGHT_PX;
        if index < filled {
            draw.add_rect([left, top], [right, bottom], ink(fill))
                .filled(true)
                .build();
        } else {
            draw.add_rect(
                [left + 0.5, top + 0.5],
                [right - 0.5, bottom - 0.5],
                ink(INK_UNAVAILABLE),
            )
            .build();
        }
    }
}
