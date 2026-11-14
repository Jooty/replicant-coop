use eframe::egui::{
    Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2,
};

use crate::app::GameMark;
use crate::app::palette::{BAND, HAIRLINE_STRONG, INK_MUTED};

const SHAPES_WIDTH_PX: f32 = 420.0;
const SHAPES_RIGHT_INSET_PX: f32 = 12.0;
const THIN_STROKE_PX: f32 = 1.0;
const FAINT_ALPHA: f32 = 0.55;
const ACCENT_LINE_ALPHA: f32 = 0.7;
const ORBIT_RADII_PX: [f32; 3] = [146.0, 112.0, 60.0];
const MOON_RADIUS_PX: f32 = 25.0;
const HORIZON_LINE_LENGTHS_PX: [f32; 5] = [380.0, 300.0, 410.0, 270.0, 340.0];
const HORIZON_LINE_GAP_PX: f32 = 10.0;
const GRID_COLUMNS: usize = 9;
const GRID_ROWS: usize = 6;
const GRID_PITCH_PX: f32 = 20.0;
const GRID_CELL_PX: f32 = 10.0;
const GRID_DOT_PX: f32 = 3.0;
const FRAME_SQUARE_PX: f32 = 170.0;
const FILLED_SQUARE_PX: f32 = 48.0;
const BAR_HEIGHTS_PX: [f32; 24] = [
    18.0, 30.0, 12.0, 40.0, 22.0, 34.0, 16.0, 44.0, 26.0, 14.0, 38.0, 20.0, 30.0, 46.0, 18.0, 28.0,
    36.0, 12.0, 24.0, 42.0, 16.0, 32.0, 20.0, 38.0,
];
const BAR_PITCH_PX: f32 = 7.0;

pub(super) fn paint_banner_shapes(
    painter: &Painter,
    banner: Rect,
    mark: GameMark,
    accent: Color32,
) {
    let right = banner.right() - SHAPES_RIGHT_INSET_PX;
    let area = Rect::from_min_max(
        pos2(right - SHAPES_WIDTH_PX, banner.top()),
        pos2(right, banner.bottom()),
    );
    let painter = painter.with_clip_rect(banner.shrink(1.0));
    match mark {
        GameMark::Circle => paint_rings(&painter, area, accent),
        GameMark::Diamond => paint_grid(&painter, area, accent),
    }
}

fn paint_rings(painter: &Painter, area: Rect, accent: Color32) {
    let centre = pos2(area.right() - 150.0, area.center().y - 18.0);
    let faint = Stroke::new(THIN_STROKE_PX, HAIRLINE_STRONG.gamma_multiply(FAINT_ALPHA));
    painter.circle_stroke(centre, ORBIT_RADII_PX[0], faint);
    painter.circle_stroke(
        centre,
        ORBIT_RADII_PX[1],
        Stroke::new(THIN_STROKE_PX, INK_MUTED.gamma_multiply(0.5)),
    );
    painter.circle_filled(centre, ORBIT_RADII_PX[2], BAND.gamma_multiply(1.4));
    painter.circle_filled(centre + vec2(30.0, -36.0), MOON_RADIUS_PX, accent);
    painter.line_segment(
        [
            pos2(centre.x - 56.0, area.top() + 20.0),
            pos2(centre.x - 56.0, centre.y + 60.0),
        ],
        faint,
    );
    painter.circle_filled(
        centre + vec2(-108.0, -62.0),
        5.0,
        accent.gamma_multiply(0.6),
    );
    paint_diamond(
        painter,
        centre + vec2(-176.0, 6.0),
        3.5,
        INK_MUTED.gamma_multiply(0.7),
    );
    paint_diamond(
        painter,
        centre + vec2(-132.0, 42.0),
        3.0,
        INK_MUTED.gamma_multiply(0.7),
    );
    let first_line_y = centre.y + 76.0;
    for (index, length) in HORIZON_LINE_LENGTHS_PX.iter().enumerate() {
        let y = first_line_y + index as f32 * HORIZON_LINE_GAP_PX;
        painter.hline((area.right() - length)..=area.right(), y, faint);
    }
}

fn paint_grid(painter: &Painter, area: Rect, accent: Color32) {
    let origin = pos2(area.left() + 40.0, area.top() + 40.0);
    let outline = Stroke::new(THIN_STROKE_PX, INK_MUTED.gamma_multiply(0.6));
    for row in 0..GRID_ROWS {
        for column in 0..GRID_COLUMNS {
            let centre = origin + vec2(column as f32, row as f32) * GRID_PITCH_PX;
            let cell = Rect::from_center_size(centre, vec2(GRID_CELL_PX, GRID_CELL_PX));
            let dot = Rect::from_center_size(centre, vec2(GRID_DOT_PX, GRID_DOT_PX));
            let _ = match (row * 7 + column * 3) % 5 {
                0 => painter.rect_filled(cell, CornerRadius::ZERO, accent.gamma_multiply(0.8)),
                1 | 3 => painter.rect_stroke(cell, CornerRadius::ZERO, outline, StrokeKind::Inside),
                _ => painter.rect_filled(dot, CornerRadius::ZERO, INK_MUTED.gamma_multiply(0.6)),
            };
        }
    }
    let frame = Rect::from_min_size(
        pos2(
            area.right() - FRAME_SQUARE_PX - 10.0,
            area.center().y - FRAME_SQUARE_PX / 2.0 + 6.0,
        ),
        vec2(FRAME_SQUARE_PX, FRAME_SQUARE_PX),
    );
    let band = [
        pos2(frame.left() + 20.0, area.bottom()),
        pos2(frame.right() - 20.0, area.top()),
        pos2(frame.right() + 20.0, area.top()),
        pos2(frame.left() + 60.0, area.bottom()),
    ];
    painter.add(Shape::convex_polygon(
        band.to_vec(),
        HAIRLINE_STRONG.gamma_multiply(0.35),
        Stroke::NONE,
    ));
    painter.rect_stroke(
        frame,
        CornerRadius::ZERO,
        Stroke::new(THIN_STROKE_PX, accent.gamma_multiply(ACCENT_LINE_ALPHA)),
        StrokeKind::Inside,
    );
    painter.rect_filled(
        Rect::from_center_size(
            frame.center() + vec2(18.0, 16.0),
            vec2(FILLED_SQUARE_PX, FILLED_SQUARE_PX),
        ),
        CornerRadius::ZERO,
        accent.gamma_multiply(0.85),
    );
    let baseline = area.bottom() - 24.0;
    for (index, height) in BAR_HEIGHTS_PX.iter().enumerate() {
        let x = area.left() + 40.0 + index as f32 * BAR_PITCH_PX;
        let color = if index % 4 == 1 {
            accent.gamma_multiply(0.7)
        } else {
            INK_MUTED.gamma_multiply(0.45)
        };
        painter.vline(x, (baseline - height)..=baseline, Stroke::new(2.0, color));
    }
}

fn paint_diamond(painter: &Painter, centre: Pos2, reach: f32, color: Color32) {
    painter.add(Shape::convex_polygon(
        vec![
            centre + vec2(0.0, -reach),
            centre + vec2(reach, 0.0),
            centre + vec2(0.0, reach),
            centre + vec2(-reach, 0.0),
        ],
        color,
        Stroke::NONE,
    ));
}
