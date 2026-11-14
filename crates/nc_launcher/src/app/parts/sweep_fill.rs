use eframe::egui::{Painter, Rect, vec2};
use nc_launcher_core::Sweep;

use crate::app::palette::{HARD_SHADOW, SAND};

/// The sand fill of a selected control, `sweep` of the way across `rect`, over its hard offset shadow.
/// Returns the part of `rect` it covers.
pub fn paint_sweep_fill(
    painter: &Painter,
    rect: Rect,
    sweep: Sweep,
    shadow_offset_px: f32,
) -> Rect {
    let (left, right) = sweep.covered(rect.left(), rect.right());
    let covered = Rect::from_x_y_ranges(left..=right, rect.y_range());
    if sweep.is_empty() {
        return covered;
    }
    painter.rect_filled(
        covered.translate(vec2(shadow_offset_px, shadow_offset_px)),
        0.0,
        HARD_SHADOW,
    );
    painter.rect_filled(covered, 0.0, SAND);
    covered
}

/// Paints a control's marks twice: with `on_sand` true clipped to the `covered` fill, and false over the rest
/// of `rect`, so the fill turns the ink over as it sweeps.
pub fn paint_in_two_inks(
    painter: &Painter,
    rect: Rect,
    covered: Rect,
    paint: impl Fn(&Painter, bool),
) {
    let whole = rect.expand(1.0);
    if covered.width() > 0.0 {
        paint(&painter.with_clip_rect(covered), true);
    }
    let left_rest = Rect::from_x_y_ranges(whole.left()..=covered.left(), whole.y_range());
    let right_rest = Rect::from_x_y_ranges(covered.right()..=whole.right(), whole.y_range());
    for rest in [left_rest, right_rest] {
        if rest.width() > 0.0 {
            paint(&painter.with_clip_rect(rest), false);
        }
    }
}
