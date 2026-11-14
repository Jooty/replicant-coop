use eframe::egui::{Response, Sense, Stroke, StrokeKind, Ui, vec2};
use nc_ui_style::LINE_BUTTON_SIDE_PADDING_PX;

use crate::app::palette::{HATCH_LINE, HOVER_WASH, INK, INK_MUTED, INK_UNAVAILABLE, NOT_READY};
use crate::app::parts::{caps_galley, paint_hatching};

pub use nc_ui_style::LINE_BUTTON_HEIGHT_PX;

/// How a line button is coloured: plain sand, or warm for an update or a warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineTone {
    Plain,
    Warm,
}

/// An outlined button with a spaced-capital label. A disabled one is hatched and greyed.
pub fn line_button(
    ui: &mut Ui,
    label: &str,
    tone: LineTone,
    height: f32,
    side_padding: Option<f32>,
    enabled: bool,
) -> Response {
    let padding = side_padding.unwrap_or(LINE_BUTTON_SIDE_PADDING_PX);
    let galley = caps_galley(ui.painter(), label, INK);
    let size = vec2(galley.size().x + 2.0 * padding, height);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let painter = ui.painter();
    let (rest_border, ink) = match (enabled, tone) {
        (false, _) => (INK_UNAVAILABLE, INK_UNAVAILABLE),
        (true, LineTone::Plain) => (INK_MUTED, INK),
        (true, LineTone::Warm) => (NOT_READY, NOT_READY),
    };
    let border = if !enabled {
        paint_hatching(painter, rect, HATCH_LINE);
        rest_border
    } else if response.hovered() {
        painter.rect_filled(rect, 0.0, HOVER_WASH);
        ink
    } else {
        rest_border
    };
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, border), StrokeKind::Inside);
    painter.galley_with_override_text_color(rect.center() - galley.size() / 2.0, galley, ink);
    response
}
