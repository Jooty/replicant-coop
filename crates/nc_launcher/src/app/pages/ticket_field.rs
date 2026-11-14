use eframe::egui::{Frame, Margin, Rect, Response, Stroke, TextEdit, Ui};

use crate::app::palette::{HAIRLINE_STRONG, INK};
use crate::app::style::body_font;

const TICKET_FIELD_SIDE_PADDING_PX: i8 = 12;
const TICKET_FIELD_TOP_PADDING_PX: i8 = 7;

pub(super) fn ticket_field(ui: &mut Ui, rect: Rect, ticket: &mut String) -> Response {
    let response = ui.put(
        rect,
        TextEdit::singleline(ticket)
            .frame(Frame::NONE)
            .margin(Margin {
                left: TICKET_FIELD_SIDE_PADDING_PX,
                right: TICKET_FIELD_SIDE_PADDING_PX,
                top: TICKET_FIELD_TOP_PADDING_PX,
                bottom: 0,
            })
            .font(body_font())
            .text_color(INK)
            .desired_width(rect.width()),
    );
    let underline = rect.bottom() - 0.5;
    ui.painter()
        .hline(rect.x_range(), underline, Stroke::new(1.0, HAIRLINE_STRONG));
    response
}
