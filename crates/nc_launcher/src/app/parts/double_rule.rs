use eframe::egui::{Painter, Rangef, Sense, Stroke, Ui, vec2};

use crate::app::motion::reveal_share;
use crate::app::palette::INK_MUTED;

pub use nc_ui_style::DOUBLE_RULE_HEIGHT_PX;

/// Two 1 px lines, 4 px apart top to bottom, that close every section and the top bar.
pub fn paint_double_rule(painter: &Painter, x_range: impl Into<Rangef>, top: f32) {
    let x_range = x_range.into();
    let stroke = Stroke::new(1.0, INK_MUTED);
    painter.hline(x_range, top + 0.5, stroke);
    painter.hline(x_range, top + DOUBLE_RULE_HEIGHT_PX - 0.5, stroke);
}

pub fn double_rule(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), DOUBLE_RULE_HEIGHT_PX),
        Sense::hover(),
    );
    // A rule coming in with its section draws from the left.
    let right = rect.left() + rect.width() * reveal_share(ui);
    paint_double_rule(ui.painter(), rect.left()..=right, rect.top());
}
