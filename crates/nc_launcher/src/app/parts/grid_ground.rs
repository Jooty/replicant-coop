use eframe::egui::{Painter, Rect, Stroke};

use crate::app::palette::{GRID_LINE, GROUND};

const GRID_CELL_PX: f32 = 24.0;
const GRID_LIFT_PX: f32 = 4.0;

/// The page ground: the dark fill and a faint 24 px grid.
pub fn paint_grid_ground(painter: &Painter, rect: Rect) {
    painter.rect_filled(rect, 0.0, GROUND);
    let stroke = Stroke::new(1.0, GRID_LINE);
    let mut y = rect.top() - GRID_LIFT_PX;
    while y < rect.bottom() {
        if y >= rect.top() {
            painter.hline(rect.x_range(), y + 0.5, stroke);
        }
        y += GRID_CELL_PX;
    }
    let mut x = rect.left();
    while x < rect.right() {
        painter.vline(x + 0.5, rect.y_range(), stroke);
        x += GRID_CELL_PX;
    }
}
