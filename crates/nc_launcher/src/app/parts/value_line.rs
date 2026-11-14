use eframe::egui::{Align2, Color32, Response, Sense, Ui, pos2, vec2};

use crate::app::parts::caps_galley;
use crate::app::style::body_font;

const VALUE_LINE_HEIGHT_PX: f32 = 30.0;
const VALUE_LINE_LABEL_LEFT_PX: f32 = 12.0;

/// How a value line's value is lettered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueLettering {
    Body,
    Capitals,
}

/// A read-only option line: the label at the left and its value at the right.
pub fn value_line(
    ui: &mut Ui,
    label: &str,
    value: &str,
    lettering: ValueLettering,
    ink: (Color32, Color32),
) -> Response {
    let (label_ink, value_ink) = ink;
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), VALUE_LINE_HEIGHT_PX),
        Sense::hover(),
    );
    let painter = ui.painter();
    let middle = rect.center().y;
    painter.text(
        pos2(rect.left() + VALUE_LINE_LABEL_LEFT_PX, middle),
        Align2::LEFT_CENTER,
        label,
        body_font(),
        label_ink,
    );
    let right = pos2(rect.right(), middle);
    match lettering {
        ValueLettering::Body => {
            painter.text(right, Align2::RIGHT_CENTER, value, body_font(), value_ink);
        }
        ValueLettering::Capitals => {
            let galley = caps_galley(painter, value, value_ink);
            let at = Align2::RIGHT_CENTER.anchor_size(right, galley.size()).min;
            painter.galley(at, galley, value_ink);
        }
    }
    response
}
