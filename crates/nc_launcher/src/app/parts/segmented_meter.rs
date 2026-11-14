use eframe::egui::{Color32, Painter, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use nc_ui_style::{METER_CELL_GAP_PX, METER_CELL_HEIGHT_PX, METER_CELL_WIDTH_PX};

use crate::app::palette::{INK_UNAVAILABLE, from_shared};
use crate::app::parts::ControlLook;

pub use nc_ui_style::METER_ROW_HEIGHT_PX;

/// A row of `cells` boxes standing in for a slider: `value` out of `max` fills them from the left. Clicking or
/// dragging over it sets the value to the cell under the pointer when the look is editable.
pub fn segmented_meter(
    ui: &mut Ui,
    value: &mut u32,
    max: u32,
    cells: u32,
    look: ControlLook,
) -> Response {
    let editable = look.editable();
    let cells = cells.max(1);
    let width = meter_width(cells);
    let sense = if editable {
        Sense::click_and_drag()
    } else {
        Sense::hover()
    };
    let (rect, mut response) = ui.allocate_exact_size(vec2(width, METER_ROW_HEIGHT_PX), sense);
    if editable
        && (response.clicked() || response.dragged())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let pitch = METER_CELL_WIDTH_PX + METER_CELL_GAP_PX;
        let cell = ((pointer.x - rect.left()) / pitch)
            .floor()
            .clamp(-1.0, cells as f32 - 1.0);
        let picked = ((cell + 1.0) as u32 * max) / cells;
        if picked != *value {
            *value = picked;
            response.mark_changed();
        }
    }
    let filled = (*value as u64 * cells as u64).div_ceil(max.max(1) as u64) as u32;
    let fill = from_shared(look.amount_ink());
    paint_meter_cells(ui.painter(), rect, cells, filled, fill);
    response
}

/// The meter's `cells` boxes across `rect`, the first `filled` of them solid in `fill`.
pub fn paint_meter_cells(painter: &Painter, rect: Rect, cells: u32, filled: u32, fill: Color32) {
    for index in 0..cells {
        let left = rect.left() + index as f32 * (METER_CELL_WIDTH_PX + METER_CELL_GAP_PX);
        let cell = Rect::from_min_size(
            pos2(left, rect.center().y - METER_CELL_HEIGHT_PX / 2.0),
            vec2(METER_CELL_WIDTH_PX, METER_CELL_HEIGHT_PX),
        );
        if index < filled {
            painter.rect_filled(cell, 0.0, fill);
        } else {
            painter.rect_stroke(
                cell,
                0.0,
                Stroke::new(1.0, INK_UNAVAILABLE),
                StrokeKind::Inside,
            );
        }
    }
}

pub fn meter_width(cells: u32) -> f32 {
    let cells = cells.max(1);
    cells as f32 * METER_CELL_WIDTH_PX + (cells - 1) as f32 * METER_CELL_GAP_PX
}
