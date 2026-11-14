use imgui::DrawListMut;
use nc_ui_style::HATCH_PERIOD_PX;

use crate::menu_view::ink;

/// Diagonal 1 px lines clipped to the box from `min` to `max`. Hatching always means "empty or not available".
pub fn paint_hatching(draw: &DrawListMut<'_>, min: [f32; 2], max: [f32; 2], color: [u8; 4]) {
    let step = HATCH_PERIOD_PX * std::f32::consts::SQRT_2;
    let width = max[0] - min[0];
    let height = max[1] - min[1];
    draw.with_clip_rect_intersect(min, max, || {
        // Lines run bottom-left to top-right, 6 px apart measured across them.
        let mut offset = -step;
        while offset < width + height {
            draw.add_line(
                [min[0] + offset - height, max[1]],
                [min[0] + offset, min[1]],
                ink(color),
            )
            .build();
            offset += step;
        }
    });
}
