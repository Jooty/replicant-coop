use imgui::{StyleColor, StyleVar, Ui};
use nc_ui_style::{
    BAND, GROUND, HAIRLINE_STRONG, HARD_SHADOW, HOVER_WASH, INK, INK_MUTED, MENU_GROUND_ALPHA,
    rgba_to_f32, rgba_with_alpha,
};

use crate::menu_view::menu_sizes::{
    MENU_FRAME_PADDING_PX, MENU_ITEM_INNER_SPACING_PX, MENU_ITEM_SPACING_PX, MENU_WINDOW_PADDING_PX,
};
use crate::menu_view::{MenuFont, with_menu_font};

const CLEAR: [u8; 4] = [0, 0, 0, 0];
const MENU_SCROLLBAR_WIDTH_PX: f32 = 6.0;
const MENU_SLIDER_GRAB_MIN_PX: f32 = 9.0;
const MENU_BORDER_PX: f32 = 1.0;

/// Draws `draw` in the launcher's look: the dark ground, sand ink, hairline borders and the running-text face.
/// Only what is drawn inside takes the look, so the HUD keeps its own.
pub fn with_menu_look<R>(ui: &Ui, draw: impl FnOnce() -> R) -> R {
    let colors = [
        (StyleColor::Text, INK),
        (StyleColor::TextDisabled, INK_MUTED),
        (
            StyleColor::WindowBg,
            rgba_with_alpha(GROUND, MENU_GROUND_ALPHA),
        ),
        (StyleColor::ChildBg, CLEAR),
        (StyleColor::PopupBg, BAND),
        (StyleColor::Border, HAIRLINE_STRONG),
        (StyleColor::BorderShadow, CLEAR),
        (StyleColor::FrameBg, CLEAR),
        (StyleColor::FrameBgHovered, HOVER_WASH),
        (StyleColor::FrameBgActive, HOVER_WASH),
        (StyleColor::TitleBg, GROUND),
        (StyleColor::TitleBgActive, BAND),
        (StyleColor::TitleBgCollapsed, GROUND),
        (StyleColor::ScrollbarBg, CLEAR),
        (StyleColor::ScrollbarGrab, HAIRLINE_STRONG),
        (StyleColor::ScrollbarGrabHovered, INK_MUTED),
        (StyleColor::ScrollbarGrabActive, INK),
        (StyleColor::CheckMark, INK),
        (StyleColor::SliderGrab, INK_MUTED),
        (StyleColor::SliderGrabActive, INK),
        (StyleColor::Button, CLEAR),
        (StyleColor::ButtonHovered, HOVER_WASH),
        (StyleColor::ButtonActive, HARD_SHADOW),
        (StyleColor::Header, HOVER_WASH),
        (StyleColor::HeaderHovered, HOVER_WASH),
        (StyleColor::HeaderActive, HARD_SHADOW),
        (StyleColor::Separator, HAIRLINE_STRONG),
        (StyleColor::SeparatorHovered, INK_MUTED),
        (StyleColor::SeparatorActive, INK),
        (StyleColor::ResizeGrip, CLEAR),
        (StyleColor::ResizeGripHovered, HAIRLINE_STRONG),
        (StyleColor::ResizeGripActive, INK_MUTED),
        (StyleColor::TextSelectedBg, HARD_SHADOW),
        (StyleColor::NavHighlight, INK),
    ];
    let color_tokens: Vec<_> = colors
        .into_iter()
        .map(|(slot, color)| ui.push_style_color(slot, rgba_to_f32(color)))
        .collect();
    let var_tokens = [
        StyleVar::WindowPadding(MENU_WINDOW_PADDING_PX),
        StyleVar::FramePadding(MENU_FRAME_PADDING_PX),
        StyleVar::ItemSpacing(MENU_ITEM_SPACING_PX),
        StyleVar::ItemInnerSpacing(MENU_ITEM_INNER_SPACING_PX),
        StyleVar::WindowBorderSize(MENU_BORDER_PX),
        StyleVar::FrameBorderSize(MENU_BORDER_PX),
        StyleVar::PopupBorderSize(MENU_BORDER_PX),
        StyleVar::ScrollbarSize(MENU_SCROLLBAR_WIDTH_PX),
        StyleVar::GrabMinSize(MENU_SLIDER_GRAB_MIN_PX),
        StyleVar::WindowRounding(0.0),
        StyleVar::FrameRounding(0.0),
        StyleVar::PopupRounding(0.0),
        StyleVar::ScrollbarRounding(0.0),
        StyleVar::GrabRounding(0.0),
    ]
    .map(|var| ui.push_style_var(var));
    let result = with_menu_font(ui, MenuFont::Body, draw);
    drop(var_tokens);
    drop(color_tokens);
    result
}
