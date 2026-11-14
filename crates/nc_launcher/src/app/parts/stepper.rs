use eframe::egui::{Align2, Rect, Response, Sense, Ui, pos2, vec2};

use crate::app::palette::{INK, INK_MUTED, INK_UNAVAILABLE};
use crate::app::parts::{paint_triangle_left, paint_triangle_right};
use crate::app::style::body_font;

const STEPPER_HEIGHT_PX: f32 = 30.0;
const STEPPER_ARROW_WIDTH_PX: f32 = 7.0;
const STEPPER_ARROW_HIT_WIDTH_PX: f32 = 20.0;
const STEPPER_GAP_PX: f32 = 12.0;
const STEPPER_LINE_LABEL_LEFT_PX: f32 = 12.0;

/// Which way a stepper was clicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepDirection {
    Back,
    Forward,
}

/// A value between two small triangles, with an optional muted note after it. A disabled stepper shows the
/// value without arrows and never reports a click.
pub fn stepper(
    ui: &mut Ui,
    value: &str,
    note: Option<&str>,
    enabled: bool,
) -> Option<StepDirection> {
    let painter = ui.painter();
    let ink = if enabled { INK } else { INK_MUTED };
    let value_galley = painter.layout_no_wrap(value.to_owned(), body_font(), ink);
    let note_galley =
        note.map(|note| painter.layout_no_wrap(note.to_owned(), body_font(), INK_MUTED));
    let arrows = if enabled {
        2.0 * (STEPPER_ARROW_WIDTH_PX + STEPPER_GAP_PX)
    } else {
        0.0
    };
    let note_width = note_galley
        .as_ref()
        .map_or(0.0, |galley| galley.size().x + STEPPER_GAP_PX);
    let width = arrows + value_galley.size().x + note_width;
    let (rect, row) = ui.allocate_exact_size(vec2(width, STEPPER_HEIGHT_PX), Sense::hover());
    let middle = rect.center().y;
    let mut left = rect.left();
    let back = enabled.then(|| arrow_hit(ui, &row, left, "stepper-back"));
    if enabled {
        left += STEPPER_ARROW_WIDTH_PX + STEPPER_GAP_PX;
    }
    let painter = ui.painter();
    painter.galley(
        pos2(left, middle - value_galley.size().y / 2.0),
        value_galley.clone(),
        ink,
    );
    left += value_galley.size().x + STEPPER_GAP_PX;
    if let Some(galley) = note_galley {
        painter.galley(
            pos2(left, middle - galley.size().y / 2.0),
            galley.clone(),
            INK_MUTED,
        );
        left += galley.size().x + STEPPER_GAP_PX;
    }
    let forward = enabled.then(|| arrow_hit(ui, &row, left, "stepper-forward"));
    let painter = ui.painter();
    let arrow_ink = |response: &Response| if response.hovered() { INK } else { INK_MUTED };
    if let Some(back) = &back {
        paint_triangle_left(painter, pos2(rect.left(), middle), arrow_ink(back));
    }
    if let Some(forward) = &forward {
        paint_triangle_right(
            painter,
            pos2(left + STEPPER_ARROW_WIDTH_PX, middle),
            arrow_ink(forward),
        );
    }
    if back.is_some_and(|response| response.clicked()) {
        Some(StepDirection::Back)
    } else if forward.is_some_and(|response| response.clicked()) {
        Some(StepDirection::Forward)
    } else {
        None
    }
}

/// A stepper on a 30 px option line: the label at the left, the stepper at the right.
pub fn stepper_line(ui: &mut Ui, label: &str, value: &str, enabled: bool) -> Option<StepDirection> {
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), STEPPER_HEIGHT_PX),
        Sense::hover(),
    );
    let label_ink = if enabled { INK } else { INK_UNAVAILABLE };
    ui.painter().text(
        pos2(rect.left() + STEPPER_LINE_LABEL_LEFT_PX, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        body_font(),
        label_ink,
    );
    let value_width = ui
        .painter()
        .layout_no_wrap(value.to_owned(), body_font(), INK)
        .size()
        .x;
    let arrows = if enabled {
        2.0 * (STEPPER_ARROW_WIDTH_PX + STEPPER_GAP_PX)
    } else {
        0.0
    };
    let left = rect.right() - value_width - arrows;
    let slot = Rect::from_min_max(pos2(left, rect.top()), rect.max);
    ui.scope_builder(eframe::egui::UiBuilder::new().max_rect(slot), |ui| {
        stepper(ui, value, None, enabled)
    })
    .inner
}

fn arrow_hit(ui: &Ui, row: &Response, left: f32, salt: &str) -> Response {
    let rect = Rect::from_min_size(
        pos2(
            left - (STEPPER_ARROW_HIT_WIDTH_PX - STEPPER_ARROW_WIDTH_PX) / 2.0,
            row.rect.top(),
        ),
        vec2(STEPPER_ARROW_HIT_WIDTH_PX, row.rect.height()),
    );
    ui.interact(rect, row.id.with(salt), Sense::click())
}
