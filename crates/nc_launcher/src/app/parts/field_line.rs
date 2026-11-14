use eframe::egui::{FontId, Frame, Margin, Response, RichText, Sense, Stroke, TextEdit, Ui, vec2};
use nc_ui_style::FIELD_LINE_HEIGHT_PX;

use crate::app::palette::{INK, INK_MUTED, INK_UNAVAILABLE};

const FIELD_LINE_RIGHT_PADDING_PX: i8 = 24;

/// An editable text on an underline, the launcher's text field: no box, the muted rule under it.
pub fn field_line(
    ui: &mut Ui,
    text: &mut String,
    width: f32,
    hint: &str,
    font: FontId,
) -> Response {
    let (rect, _) = ui.allocate_exact_size(vec2(width, FIELD_LINE_HEIGHT_PX), Sense::hover());
    let response = ui.put(
        rect,
        TextEdit::singleline(text)
            .frame(Frame::NONE)
            .margin(Margin {
                left: 0,
                right: FIELD_LINE_RIGHT_PADDING_PX,
                top: 2,
                bottom: 0,
            })
            .font(font)
            .text_color(INK)
            .hint_text(RichText::new(hint).color(INK_UNAVAILABLE))
            .desired_width(width),
    );
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, INK_MUTED),
    );
    response
}
