use eframe::egui::{Color32, Painter, Rect, Stroke, pos2};
use nc_ui_style::HATCH_PERIOD_PX;

pub const HATCH_STEP_ALONG_PX: f32 = HATCH_PERIOD_PX * std::f32::consts::SQRT_2;

/// Diagonal 1 px lines clipped to `rect`. Hatching always means "empty or not available".
pub fn paint_hatching(painter: &Painter, rect: Rect, color: Color32) {
    paint_hatching_shifted(painter, rect, color, 0.0);
}

/// The same hatching with its lines moved `shift_px` to the right, for a slow drift.
pub fn paint_hatching_shifted(painter: &Painter, rect: Rect, color: Color32, shift_px: f32) {
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    // Lines run bottom-left to top-right, 6 px apart measured across them.
    let step = HATCH_STEP_ALONG_PX;
    let stroke = Stroke::new(1.0, color);
    let mut offset = shift_px.rem_euclid(step) - step;
    while offset < rect.width() + rect.height() {
        let start = pos2(rect.left() + offset - rect.height(), rect.bottom());
        let end = pos2(rect.left() + offset, rect.top());
        painter.line_segment([start, end], stroke);
        offset += step;
    }
}
