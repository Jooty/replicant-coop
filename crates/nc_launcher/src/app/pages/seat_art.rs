use eframe::egui::epaint::RectShape;
use eframe::egui::{Color32, Mesh, Painter, Rect, TextureHandle, pos2};
use nc_launcher_core::{SeatArt, crop_share_around_focus};

use crate::app::palette::{BAND, GROUND};

const SEAT_FADE_STOPS: [(f32, f32); 5] = [
    (0.0, 0.16),
    (0.42, 0.24),
    (0.68, 0.74),
    (0.86, 0.94),
    (1.0, 1.0),
];

pub(super) fn paint_seat_art(
    painter: &Painter,
    rect: Rect,
    art: Option<&TextureHandle>,
    seat: SeatArt,
    shown: f32,
) {
    let Some(art) = art else {
        painter.rect_filled(rect, 0.0, BAND.gamma_multiply(shown));
        painter.add(fade_mesh(rect));
        return;
    };
    let size = art.size_vec2();
    let crop = crop_share_around_focus(
        (size.x, size.y),
        (rect.width(), rect.height()),
        seat.focus,
        seat.height_share,
    );
    let (left_u, right_u) = if seat.mirrored {
        (crop.max_u, crop.min_u)
    } else {
        (crop.min_u, crop.max_u)
    };
    let uv = Rect::from_min_max(pos2(left_u, crop.min_v), pos2(right_u, crop.max_v));
    let tint = Color32::WHITE.gamma_multiply(shown);
    painter.add(RectShape::filled(rect, 0.0, tint).with_texture(art.id(), uv));
    painter.add(fade_mesh(rect));
}

fn fade_mesh(rect: Rect) -> Mesh {
    let mut mesh = Mesh::default();
    for (index, (at, alpha)) in SEAT_FADE_STOPS.into_iter().enumerate() {
        let y = rect.top() + at * rect.height();
        let color = GROUND.gamma_multiply(alpha);
        mesh.colored_vertex(pos2(rect.left(), y), color);
        mesh.colored_vertex(pos2(rect.right(), y), color);
        if index > 0 {
            let bottom_left = (index * 2) as u32;
            mesh.add_triangle(bottom_left - 2, bottom_left - 1, bottom_left);
            mesh.add_triangle(bottom_left - 1, bottom_left, bottom_left + 1);
        }
    }
    mesh
}
