use std::path::PathBuf;

use hudhook::imgui::{Context, FontConfig, FontGlyphRanges, FontSource, StyleColor};
use nc_ui_style as shared;

use crate::menu_view::add_menu_fonts;

pub fn apply_shared_style(context: &mut Context) {
    context.set_ini_filename(None::<PathBuf>);
    apply_colors_and_sizes(context);
    load_shared_font(context);
    add_menu_fonts(context);
}

fn apply_colors_and_sizes(context: &mut Context) {
    let style = context.style_mut();
    let window_background = shared::rgba_with_alpha(
        shared::WINDOW_BACKGROUND,
        shared::OVERLAY_WINDOW_BACKGROUND_ALPHA,
    );
    let colors = [
        (StyleColor::Text, shared::TEXT),
        (StyleColor::TextDisabled, shared::DISABLED),
        (StyleColor::WindowBg, window_background),
        (StyleColor::ChildBg, [0, 0, 0, 0]),
        (StyleColor::PopupBg, shared::WINDOW_BACKGROUND),
        (StyleColor::Border, shared::BORDER),
        (StyleColor::BorderShadow, [0, 0, 0, 0]),
        (StyleColor::FrameBg, shared::FRAME_BACKGROUND),
        (StyleColor::FrameBgHovered, shared::FRAME_HOVERED),
        (StyleColor::FrameBgActive, shared::FRAME_ACTIVE),
        (StyleColor::TitleBg, shared::INPUT_BACKGROUND),
        (StyleColor::TitleBgActive, shared::FRAME_BACKGROUND),
        (StyleColor::TitleBgCollapsed, shared::INPUT_BACKGROUND),
        (StyleColor::MenuBarBg, shared::STRIPE_BACKGROUND),
        (StyleColor::ScrollbarBg, shared::INPUT_BACKGROUND),
        (StyleColor::ScrollbarGrab, shared::BUTTON),
        (StyleColor::ScrollbarGrabHovered, shared::BUTTON_HOVERED),
        (StyleColor::ScrollbarGrabActive, shared::BUTTON_ACTIVE),
        (StyleColor::CheckMark, shared::TEXT),
        (StyleColor::SliderGrab, shared::BUTTON_HOVERED),
        (StyleColor::SliderGrabActive, shared::BUTTON_ACTIVE),
        (StyleColor::Button, shared::BUTTON),
        (StyleColor::ButtonHovered, shared::BUTTON_HOVERED),
        (StyleColor::ButtonActive, shared::BUTTON_ACTIVE),
        (StyleColor::Header, shared::BUTTON),
        (StyleColor::HeaderHovered, shared::BUTTON_HOVERED),
        (StyleColor::HeaderActive, shared::BUTTON_ACTIVE),
        (StyleColor::Separator, shared::BORDER),
        (StyleColor::SeparatorHovered, shared::FRAME_HOVERED),
        (StyleColor::SeparatorActive, shared::FRAME_ACTIVE),
        (StyleColor::ResizeGrip, shared::BUTTON),
        (StyleColor::ResizeGripHovered, shared::BUTTON_HOVERED),
        (StyleColor::ResizeGripActive, shared::BUTTON_ACTIVE),
        (StyleColor::Tab, shared::BUTTON),
        (StyleColor::TabHovered, shared::BUTTON_HOVERED),
        (StyleColor::TabActive, shared::BUTTON_ACTIVE),
        (StyleColor::TextSelectedBg, shared::FRAME_BACKGROUND),
    ];
    for (slot, color) in colors {
        style[slot] = shared::rgba_to_f32(color);
    }
    style.window_rounding = shared::ROUNDING_PX;
    style.frame_rounding = shared::ROUNDING_PX;
    style.child_rounding = shared::ROUNDING_PX;
    style.popup_rounding = shared::ROUNDING_PX;
    style.scrollbar_rounding = shared::ROUNDING_PX;
    style.grab_rounding = shared::ROUNDING_PX;
    style.tab_rounding = shared::ROUNDING_PX;
    style.window_border_size = shared::BORDER_SIZE_PX;
    style.frame_border_size = shared::BORDER_SIZE_PX;
    style.item_spacing = shared::ITEM_SPACING_PX;
    style.frame_padding = shared::FRAME_PADDING_PX;
}

fn load_shared_font(context: &mut Context) {
    let config = FontConfig {
        glyph_ranges: FontGlyphRanges::from_slice(&shared::FONT_GLYPH_RANGES),
        ..FontConfig::default()
    };
    context.fonts().add_font(&[FontSource::TtfData {
        data: shared::font_bytes(),
        size_pixels: shared::FONT_SIZE_PX,
        config: Some(config),
    }]);
}
