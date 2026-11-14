use std::sync::Arc;

use eframe::egui::text::LayoutJob;
use eframe::egui::{Color32, FontId, Galley, Painter, Response, Sense, TextFormat, Ui};

use crate::app::style::{CAPS_LETTER_SPACING_EM, caps_font};

/// One line of text with extra space between letters; `spacing_em` is a share of the font size.
pub fn spaced_job(text: &str, font: FontId, color: Color32, spacing_em: f32) -> LayoutJob {
    let format = TextFormat {
        extra_letter_spacing: font.size * spacing_em,
        font_id: font,
        color,
        ..TextFormat::default()
    };
    let mut job = LayoutJob::single_section(text.to_owned(), format);
    job.wrap.max_width = f32::INFINITY;
    job
}

pub fn caps_galley(painter: &Painter, text: &str, color: Color32) -> Arc<Galley> {
    painter.layout_job(spaced_job(
        &text.to_uppercase(),
        caps_font(),
        color,
        CAPS_LETTER_SPACING_EM,
    ))
}

/// A small label in spaced capitals, the launcher's key and button lettering.
pub fn caps_label(ui: &mut Ui, text: &str, color: Color32) -> Response {
    let galley = caps_galley(ui.painter(), text, color);
    let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().galley(rect.min, galley, color);
    response
}
