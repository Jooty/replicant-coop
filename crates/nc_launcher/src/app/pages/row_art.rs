use eframe::egui::epaint::RectShape;
use eframe::egui::{Color32, Mesh, Painter, Rect, TextureHandle, pos2};
use nc_launcher_core::{ArtFocus, crop_around_focus, fade_alpha};

use crate::app::palette::GROUND;

const FADE_SOLID_PX: f32 = 96.0;
const FADE_EASE_START: f32 = 0.35;
const FOCUS_CLEAR_FROM_PX: f32 = 280.0;
const FADE_FLOOR_ALPHA: f32 = 0.22;
const POINTED_FADE_FLOOR_ALPHA: f32 = 0.06;
const POINTED_RAMP_LIFT: f32 = 0.2;
const FADE_VERTEX_COLUMNS: usize = 17;
const FOCUS_SHARE_OF_CLEAR_PART: f32 = 0.6;
const FOCUS_ANCHOR_Y: f32 = 0.5;

/// How a row's art is lit: how far it has faded up since it loaded, and how far it is brightened under the
/// pointer, each from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct RowArtLight {
    pub shown: f32,
    pub brightened: f32,
}

pub(super) fn paint_row_art(
    painter: &Painter,
    art_rect: Rect,
    art: &TextureHandle,
    focus: ArtFocus,
    light: RowArtLight,
) {
    if art_rect.width() <= FADE_SOLID_PX {
        return;
    }
    let clear_left = FOCUS_CLEAR_FROM_PX.min(art_rect.width());
    let focus_x = clear_left + (art_rect.width() - clear_left).max(0.0) * FOCUS_SHARE_OF_CLEAR_PART;
    let anchor = ((focus_x / art_rect.width()).min(1.0), FOCUS_ANCHOR_Y);
    let image_size = art.size_vec2();
    let crop = crop_around_focus(
        (image_size.x, image_size.y),
        (art_rect.width(), art_rect.height()),
        focus,
        anchor,
    );
    let uv = Rect::from_min_max(pos2(crop.min_u, crop.min_v), pos2(crop.max_u, crop.max_v));
    let tint = Color32::WHITE.gamma_multiply(light.shown);
    painter.add(RectShape::filled(art_rect, 0.0, tint).with_texture(art.id(), uv));
    painter.add(cover_mesh(art_rect, light.brightened));
}

fn cover_mesh(art_rect: Rect, brightened: f32) -> Mesh {
    let floor = FADE_FLOOR_ALPHA + (POINTED_FADE_FLOOR_ALPHA - FADE_FLOOR_ALPHA) * brightened;
    let ramp_width = art_rect.width() - FADE_SOLID_PX;
    let mut columns = vec![(art_rect.left(), 1.0)];
    let last = FADE_VERTEX_COLUMNS - 1;
    for index in 0..=last {
        let at = index as f32 / last as f32;
        // The ramp starts part way into the eased curve, so the cover begins to thin at once.
        let eased = fade_alpha(FADE_EASE_START + (1.0 - FADE_EASE_START) * at)
            / fade_alpha(FADE_EASE_START);
        let x = art_rect.left() + FADE_SOLID_PX + at * ramp_width;
        // Under the pointer the cover thins further the farther it is from the text.
        let lift = 1.0 - POINTED_RAMP_LIFT * brightened * at;
        columns.push((x, floor + (1.0 - floor) * eased * lift));
    }
    let mut mesh = Mesh::default();
    for (index, (x, alpha)) in columns.into_iter().enumerate() {
        let color = GROUND.gamma_multiply(alpha);
        mesh.colored_vertex(pos2(x, art_rect.top()), color);
        mesh.colored_vertex(pos2(x, art_rect.bottom()), color);
        if index > 0 {
            let top_right = (index * 2) as u32;
            mesh.add_triangle(top_right - 2, top_right - 1, top_right);
            mesh.add_triangle(top_right - 1, top_right, top_right + 1);
        }
    }
    mesh
}
