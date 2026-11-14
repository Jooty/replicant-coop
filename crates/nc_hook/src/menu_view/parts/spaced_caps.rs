use imgui::{DrawListMut, Ui};

use crate::menu_view::{MenuFont, ink, with_menu_font};

/// The size of `text` in spaced capitals; `font` is `MenuFont::Caps` or `MenuFont::Heading`.
pub fn caps_size(ui: &Ui, text: &str, font: MenuFont) -> [f32; 2] {
    let upper = text.to_uppercase();
    with_menu_font(ui, font, || ui.calc_text_size(&upper))
}

/// Draws `text` in spaced capitals with its top-left corner at `at`.
pub fn paint_caps(
    ui: &Ui,
    draw: &DrawListMut<'_>,
    at: [f32; 2],
    text: &str,
    font: MenuFont,
    color: [u8; 4],
) {
    let upper = text.to_uppercase();
    with_menu_font(ui, font, || draw.add_text(at, ink(color), &upper));
}
