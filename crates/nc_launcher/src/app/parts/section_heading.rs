use eframe::egui::{Align2, FontId, Sense, Ui, pos2, vec2};
use nc_ui_style::{
    HEADING_ROW_HEIGHT_PX, HEADING_SQUARE_SIDE_PX, HEADING_SQUARE_TO_TEXT_PX,
    HEADING_TO_RULE_GAP_PX,
};

use crate::app::palette::{INK, INK_MUTED};
use crate::app::parts::{double_rule, paint_square};
use crate::app::style::{CAPS_LETTER_SPACING_EM, caps_font, medium_family};

use super::spaced_caps::{caps_galley, spaced_job};

/// A section's heading: a small solid square, the title in spaced medium capitals, an optional muted note at the
/// right, then the doubled rule.
pub fn section_heading(ui: &mut Ui, title: &str, right_note: Option<&str>) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        draw_heading(ui, title, right_note);
    });
}

fn draw_heading(ui: &mut Ui, title: &str, right_note: Option<&str>) {
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), HEADING_ROW_HEIGHT_PX),
        Sense::hover(),
    );
    let painter = ui.painter();
    let middle = rect.center().y;
    paint_square(
        painter,
        pos2(rect.left() + HEADING_SQUARE_SIDE_PX / 2.0, middle),
        HEADING_SQUARE_SIDE_PX,
        INK,
    );
    let font = caps_font();
    let title_font = FontId::new(font.size, medium_family());
    let title = painter.layout_job(spaced_job(
        &title.to_uppercase(),
        title_font,
        INK,
        CAPS_LETTER_SPACING_EM,
    ));
    let text_left = rect.left() + HEADING_SQUARE_SIDE_PX + HEADING_SQUARE_TO_TEXT_PX;
    painter.galley(pos2(text_left, middle - title.size().y / 2.0), title, INK);
    if let Some(note) = right_note {
        let galley = caps_galley(painter, note, INK_MUTED);
        let at = Align2::RIGHT_CENTER
            .anchor_size(pos2(rect.right(), middle), galley.size())
            .min;
        painter.galley(at, galley, INK_MUTED);
    }
    ui.add_space(HEADING_TO_RULE_GAP_PX);
    double_rule(ui);
}
