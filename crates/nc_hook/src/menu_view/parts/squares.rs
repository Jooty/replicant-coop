use imgui::DrawListMut;

use crate::menu_view::ink;

/// A solid square of side `side` centred on `center`, the launcher's bullet and "on" mark.
pub fn paint_square(draw: &DrawListMut<'_>, center: [f32; 2], side: f32, color: [u8; 4]) {
    let half = side / 2.0;
    draw.add_rect(
        [center[0] - half, center[1] - half],
        [center[0] + half, center[1] + half],
        ink(color),
    )
    .filled(true)
    .build();
}

/// The same square drawn as a 1 px outline, the "off" mark.
pub fn paint_open_square(draw: &DrawListMut<'_>, center: [f32; 2], side: f32, color: [u8; 4]) {
    let half = side / 2.0;
    draw.add_rect(
        [center[0] - half + 0.5, center[1] - half + 0.5],
        [center[0] + half - 0.5, center[1] + half - 0.5],
        ink(color),
    )
    .build();
}
