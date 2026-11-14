use std::sync::Arc;

use eframe::egui::{
    Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, RichText, Stroke, Style,
    TextStyle, Theme, ThemePreference, Vec2, Visuals,
};

use nc_ui_style::{
    BODY_FONT_SIZE_PX, CAPS_FONT_SIZE_PX, SANS_LIGHT_FONT_BYTES, SANS_MEDIUM_FONT_BYTES,
    SANS_REGULAR_FONT_BYTES,
};

use super::palette;

pub use nc_ui_style::CAPS_LETTER_SPACING_EM;

const GARAMOND_FONT_NAME: &str = "EB Garamond";
const GARAMOND_FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/EBGaramond-Variable.ttf");
const SANS_REGULAR_FONT_NAME: &str = "Noto Sans JP Latin";
const SANS_LIGHT_FONT_NAME: &str = "Noto Sans JP Latin Light";
const SANS_MEDIUM_FONT_NAME: &str = "Noto Sans JP Latin Medium";
const PATH_SIZE_PX: f32 = 13.0;
const MONOSPACE_SIZE_PX: f32 = 13.0;
const BORDER_WIDTH_PX: f32 = 1.0;
const ITEM_SPACING_PX: Vec2 = Vec2::new(8.0, 6.0);
const BUTTON_PADDING_PX: Vec2 = Vec2::new(16.0, 6.0);
const INTERACT_HEIGHT_PX: f32 = 28.0;

pub fn launcher_visuals() -> Visuals {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = palette::GROUND;
    visuals.window_fill = palette::BAND;
    visuals.window_stroke = Stroke::new(BORDER_WIDTH_PX, palette::HAIRLINE_STRONG);
    visuals.extreme_bg_color = palette::BAND;
    visuals.faint_bg_color = palette::BAND;
    visuals.code_bg_color = palette::BAND;
    visuals.override_text_color = Some(palette::INK);
    visuals.warn_fg_color = palette::NOT_READY;
    visuals.error_fg_color = palette::NOT_READY;
    visuals.hyperlink_color = palette::INK;
    visuals.selection.bg_fill = palette::HARD_SHADOW;
    visuals.selection.stroke = Stroke::new(BORDER_WIDTH_PX, palette::INK);
    visuals.window_corner_radius = CornerRadius::ZERO;
    visuals.menu_corner_radius = CornerRadius::ZERO;
    visuals.window_shadow.offset = [5, 5];
    visuals.window_shadow.blur = 0;
    visuals.window_shadow.spread = 0;
    visuals.window_shadow.color = palette::HARD_SHADOW;
    visuals.popup_shadow = visuals.window_shadow;
    let states = [
        (
            &mut visuals.widgets.noninteractive,
            palette::GROUND,
            palette::HAIRLINE,
        ),
        (
            &mut visuals.widgets.inactive,
            palette::BAND,
            palette::INK_MUTED,
        ),
        (
            &mut visuals.widgets.hovered,
            palette::HOVER_WASH,
            palette::INK,
        ),
        (
            &mut visuals.widgets.active,
            palette::HARD_SHADOW,
            palette::INK,
        ),
        (&mut visuals.widgets.open, palette::BAND, palette::INK),
    ];
    for (widget, fill, border) in states {
        widget.corner_radius = CornerRadius::ZERO;
        widget.weak_bg_fill = fill;
        widget.bg_fill = fill;
        widget.bg_stroke = Stroke::new(BORDER_WIDTH_PX, border);
        widget.fg_stroke = Stroke::new(BORDER_WIDTH_PX, palette::INK);
        widget.expansion = 0.0;
    }
    visuals
}

pub fn garamond_family() -> FontFamily {
    FontFamily::Name(GARAMOND_FONT_NAME.into())
}

pub fn light_family() -> FontFamily {
    FontFamily::Name(SANS_LIGHT_FONT_NAME.into())
}

pub fn medium_family() -> FontFamily {
    FontFamily::Name(SANS_MEDIUM_FONT_NAME.into())
}

/// A small heading for pages not yet drawn with the section heading: medium capitals.
pub fn heading_text(text: impl Into<String>) -> RichText {
    RichText::new(text.into().to_uppercase()).font(FontId::new(CAPS_FONT_SIZE_PX, medium_family()))
}

pub fn body_font() -> FontId {
    FontId::proportional(BODY_FONT_SIZE_PX)
}

pub fn caps_font() -> FontId {
    FontId::proportional(CAPS_FONT_SIZE_PX)
}

pub fn small_font() -> FontId {
    FontId::proportional(PATH_SIZE_PX)
}

pub fn apply_launcher_look(ctx: &Context) {
    install_launcher_fonts(ctx);
    ctx.set_theme(ThemePreference::Dark);
    ctx.set_visuals_of(Theme::Dark, launcher_visuals());
    ctx.all_styles_mut(apply_launcher_spacing);
}

fn install_launcher_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    let faces = [
        (GARAMOND_FONT_NAME, GARAMOND_FONT_BYTES),
        (SANS_REGULAR_FONT_NAME, SANS_REGULAR_FONT_BYTES),
        (SANS_LIGHT_FONT_NAME, SANS_LIGHT_FONT_BYTES),
        (SANS_MEDIUM_FONT_NAME, SANS_MEDIUM_FONT_BYTES),
    ];
    for (name, bytes) in faces {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    }
    // Every named face falls back to the default proportional list for glyphs the Latin subset lacks.
    let fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for (family, name) in [
        (garamond_family(), GARAMOND_FONT_NAME),
        (light_family(), SANS_LIGHT_FONT_NAME),
        (medium_family(), SANS_MEDIUM_FONT_NAME),
    ] {
        let mut list = vec![name.to_owned()];
        list.extend(fallback.iter().cloned());
        fonts.families.insert(family, list);
    }
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, SANS_REGULAR_FONT_NAME.to_owned());
    ctx.set_fonts(fonts);
}

fn apply_launcher_spacing(style: &mut Style) {
    style.spacing.item_spacing = ITEM_SPACING_PX;
    style.spacing.button_padding = BUTTON_PADDING_PX;
    style.spacing.interact_size.y = INTERACT_HEIGHT_PX;
    style.text_styles = [
        (TextStyle::Small, small_font()),
        (TextStyle::Body, body_font()),
        (TextStyle::Button, caps_font()),
        (
            TextStyle::Heading,
            FontId::new(CAPS_FONT_SIZE_PX, medium_family()),
        ),
        (TextStyle::Monospace, FontId::monospace(MONOSPACE_SIZE_PX)),
    ]
    .into();
}
