use eframe::egui::{Response, Sense, Ui};

use crate::app::palette::{INK, INK_MUTED, INK_UNAVAILABLE};
use crate::app::parts::caps_galley;

/// A quiet control that is only its spaced-capital label: muted at rest, full ink under the pointer.
pub fn text_control(ui: &mut Ui, label: &str, enabled: bool) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let size = caps_galley(ui.painter(), label, INK).size();
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let ink = match (enabled, response.hovered()) {
        (false, _) => INK_UNAVAILABLE,
        (true, true) => INK,
        (true, false) => INK_MUTED,
    };
    let galley = caps_galley(ui.painter(), label, ink);
    ui.painter().galley(rect.min, galley, ink);
    response
}
