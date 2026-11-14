use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2, pos2};

const TRIANGLE_HALF_HEIGHT_PX: f32 = 5.0;
const TRIANGLE_WIDTH_PX: f32 = 7.0;

/// A solid square of side `side` centred on `center`, the launcher's bullet and "on" mark.
pub fn paint_square(painter: &Painter, center: Pos2, side: f32, color: Color32) {
    painter.rect_filled(
        Rect::from_center_size(center, Vec2::splat(side)),
        0.0,
        color,
    );
}

pub fn paint_open_square(painter: &Painter, center: Pos2, side: f32, color: Color32) {
    painter.rect_stroke(
        Rect::from_center_size(center, Vec2::splat(side)),
        0.0,
        Stroke::new(1.0, color),
        StrokeKind::Inside,
    );
}

/// A small triangle pointing left whose tip sits at `tip`.
pub fn paint_triangle_left(painter: &Painter, tip: Pos2, color: Color32) {
    painter.add(Shape::convex_polygon(
        vec![
            tip,
            pos2(tip.x + TRIANGLE_WIDTH_PX, tip.y - TRIANGLE_HALF_HEIGHT_PX),
            pos2(tip.x + TRIANGLE_WIDTH_PX, tip.y + TRIANGLE_HALF_HEIGHT_PX),
        ],
        color,
        Stroke::NONE,
    ));
}

pub fn paint_triangle_right(painter: &Painter, tip: Pos2, color: Color32) {
    painter.add(Shape::convex_polygon(
        vec![
            pos2(tip.x - TRIANGLE_WIDTH_PX, tip.y - TRIANGLE_HALF_HEIGHT_PX),
            tip,
            pos2(tip.x - TRIANGLE_WIDTH_PX, tip.y + TRIANGLE_HALF_HEIGHT_PX),
        ],
        color,
        Stroke::NONE,
    ));
}
