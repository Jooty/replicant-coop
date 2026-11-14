use std::sync::atomic::{AtomicUsize, Ordering};

use imgui::{Context, FontConfig, FontGlyphRanges, FontSource, Ui};
use nc_ui_style::{
    CAPS_LETTER_SPACING_EM, FONT_GLYPH_RANGES, SANS_MEDIUM_FONT_BYTES, SANS_REGULAR_FONT_BYTES,
    font_bytes,
};

use crate::menu_view::menu_sizes::{MENU_BODY_FONT_SIZE_PX, MENU_CAPS_FONT_SIZE_PX};

const SANS_LINE_HEIGHT_PER_EM: f32 = 1.448;
const DEFAULT_FACE_LINE_HEIGHT_PER_EM: f32 = 1.121;

static FIRST_MENU_FONT_INDEX: AtomicUsize = AtomicUsize::new(usize::MAX);

/// The menu's three letterings: running text, spaced capitals for keys and buttons, and spaced medium
/// capitals for headings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuFont {
    Body,
    Caps,
    Heading,
}

impl MenuFont {
    fn atlas_offset(self) -> usize {
        match self {
            Self::Body => 0,
            Self::Caps => 1,
            Self::Heading => 2,
        }
    }
}

/// Adds the launcher's sans faces to the atlas after the fonts already there, so the default font (the one the
/// HUD draws with) stays first. Glyphs the Latin faces lack come from the default face. The launcher's sizes are
/// em sizes and the atlas takes line heights, hence the per-face factors.
pub fn add_menu_fonts(context: &mut Context) {
    let atlas = context.fonts();
    FIRST_MENU_FONT_INDEX.store(atlas.fonts().len(), Ordering::Relaxed);
    let caps_spacing = MENU_CAPS_FONT_SIZE_PX * CAPS_LETTER_SPACING_EM;
    let faces = [
        (SANS_REGULAR_FONT_BYTES, MENU_BODY_FONT_SIZE_PX, 0.0),
        (
            SANS_REGULAR_FONT_BYTES,
            MENU_CAPS_FONT_SIZE_PX,
            caps_spacing,
        ),
        (SANS_MEDIUM_FONT_BYTES, MENU_CAPS_FONT_SIZE_PX, caps_spacing),
    ];
    for (bytes, em, spacing) in faces {
        let config = || FontConfig {
            glyph_ranges: FontGlyphRanges::from_slice(&FONT_GLYPH_RANGES),
            glyph_extra_spacing: [spacing, 0.0],
            ..FontConfig::default()
        };
        atlas.add_font(&[
            FontSource::TtfData {
                data: bytes,
                size_pixels: em * SANS_LINE_HEIGHT_PER_EM,
                config: Some(config()),
            },
            FontSource::TtfData {
                data: font_bytes(),
                size_pixels: em * DEFAULT_FACE_LINE_HEIGHT_PER_EM,
                config: Some(config()),
            },
        ]);
    }
}

/// Runs `draw` with `font` pushed, or with the current font when the menu fonts were never added.
pub fn with_menu_font<R>(ui: &Ui, font: MenuFont, draw: impl FnOnce() -> R) -> R {
    let first = FIRST_MENU_FONT_INDEX.load(Ordering::Relaxed);
    let id = first
        .checked_add(font.atlas_offset())
        .and_then(|index| ui.fonts().fonts().get(index).copied());
    let token = id.map(|id| ui.push_font(id));
    let result = draw();
    drop(token);
    result
}
