use eframe::egui::text::LayoutJob;
use eframe::egui::{
    Color32, FontId, Id, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind, TextFormat, Ui,
    pos2, vec2,
};
use nc_launcher_core::{COPIED_HOLD_SECONDS, copied_shown};

use crate::app::motion::{sweep_toward, with_clock};
use crate::app::palette::{GROUND, HAIRLINE_STRONG, HOVER_WASH, INK, INK_MUTED, INK_ON_SAND};
use crate::app::parts::{caps_galley, paint_in_two_inks, paint_square, paint_sweep_fill};

const COPY_LINE_SIDE_PADDING_PX: f32 = 12.0;
const COPY_LINE_TOP_PADDING_PX: f32 = 6.0;
const COPY_LINE_BOTTOM_PADDING_PX: f32 = 5.0;
const COPY_LINE_TEXT_TO_MARK_PX: f32 = 16.0;
const COPY_MARK_BOX_PX: f32 = 13.0;
const COPY_MARK_SQUARE_PX: f32 = 9.0;
const COPY_MARK_OFFSET_PX: f32 = 4.0;
const COPIED_SQUARE_SIDE_PX: f32 = 7.0;
const COPIED_SQUARE_TO_TEXT_PX: f32 = 9.0;
const COPIED_SHADOW_OFFSET_PX: f32 = 4.0;
const COPIED_LABEL: &str = "Copied";
const COPIED_SWEEP_SALT: &str = "copied-sweep";

/// A line of text that is its own copy control: underlined at rest, outlined on hover. On click it copies
/// `copy_value` and a sand fill sweeps over it reading COPIED for a moment, then sweeps away. Each line of
/// `text` stays one row.
pub fn copy_line(
    ui: &mut Ui,
    id_salt: &str,
    text: &str,
    copy_value: &str,
    width: f32,
    font: FontId,
) -> Response {
    copy_line_inked(ui, id_salt, text, copy_value, width, font, INK)
}

/// A copy line whose text rests in `ink` instead of the plain ink.
pub fn copy_line_inked(
    ui: &mut Ui,
    id_salt: &str,
    text: &str,
    copy_value: &str,
    width: f32,
    font: FontId,
    ink: Color32,
) -> Response {
    let id = copy_line_id(id_salt);
    let now = ui.input(|input| input.time);
    let copied_at: Option<f64> = ui.data(|data| data.get_temp(id));
    let shown = copied_at.is_some_and(|at| copied_shown((now - at) as f32));
    let sweep = sweep_toward(ui, id.with(COPIED_SWEEP_SALT), shown);
    let right_part = if sweep.is_empty() {
        COPY_MARK_BOX_PX
    } else {
        caps_galley(ui.painter(), COPIED_LABEL, INK_ON_SAND)
            .size()
            .x
            + COPIED_SQUARE_TO_TEXT_PX
            + COPIED_SQUARE_SIDE_PX
    };
    let text_width =
        width - 2.0 * COPY_LINE_SIDE_PADDING_PX - COPY_LINE_TEXT_TO_MARK_PX - right_part;
    let lines: Vec<_> = text
        .lines()
        .map(|line| {
            ui.painter()
                .layout_job(elided_line_job(line, font.clone(), INK, text_width))
        })
        .collect();
    let text_height: f32 = lines.iter().map(|line| line.size().y).sum();
    let height = text_height + COPY_LINE_TOP_PADDING_PX + COPY_LINE_BOTTOM_PADDING_PX;
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    if response.clicked() {
        ui.ctx().copy_text(copy_value.to_owned());
        ui.data_mut(|data| data.insert_temp(id, now));
    }
    if shown && let Some(at) = copied_at {
        // Sleep through the hold; the sweep back asks for its own frames.
        let hold_left = COPIED_HOLD_SECONDS - (now - at) as f32;
        with_clock(ui, |clock, _| clock.wake_in(hold_left));
    }
    let hovered = response.hovered();
    let painter = ui.painter();
    if !sweep.is_full() {
        paint_rest_frame(painter, rect, hovered);
    }
    let covered = paint_sweep_fill(painter, rect, sweep, COPIED_SHADOW_OFFSET_PX);
    let right_middle = pos2(rect.right() - COPY_LINE_SIDE_PADDING_PX, rect.center().y);
    let text_left = rect.left() + COPY_LINE_SIDE_PADDING_PX;
    paint_in_two_inks(painter, rect, covered, |painter, on_sand| {
        let ink = if on_sand { INK_ON_SAND } else { ink };
        let mut line_top = rect.top() + COPY_LINE_TOP_PADDING_PX;
        for line in &lines {
            painter.galley_with_override_text_color(pos2(text_left, line_top), line.clone(), ink);
            line_top += line.size().y;
        }
        if on_sand {
            paint_copied_label(painter, right_middle);
        } else {
            let mark = if hovered { INK } else { INK_MUTED };
            paint_copy_mark(painter, right_middle, mark);
        }
    });
    response
}

/// Where a copy line keeps the time of its last click.
pub fn copy_line_id(id_salt: &str) -> Id {
    Id::unique(("copy-line", id_salt))
}

fn elided_line_job(text: &str, font: FontId, color: Color32, width: f32) -> LayoutJob {
    let mut job = LayoutJob::single_section(
        text.to_owned(),
        TextFormat {
            font_id: font,
            color,
            ..TextFormat::default()
        },
    );
    // Each given line stays one row; a line too long for the width ends in an ellipsis.
    job.wrap.max_width = width;
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('\u{2026}');
    job
}

fn paint_rest_frame(painter: &Painter, rect: Rect, hovered: bool) {
    if hovered {
        painter.rect_filled(rect, 0.0, HOVER_WASH);
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, INK), StrokeKind::Inside);
    } else {
        painter.hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            Stroke::new(1.0, HAIRLINE_STRONG),
        );
    }
}

fn paint_copy_mark(painter: &Painter, right_middle: Pos2, color: Color32) {
    let back = Rect::from_min_size(
        pos2(
            right_middle.x - COPY_MARK_BOX_PX,
            right_middle.y - COPY_MARK_BOX_PX / 2.0,
        ),
        vec2(COPY_MARK_SQUARE_PX, COPY_MARK_SQUARE_PX),
    );
    painter.rect_stroke(back, 0.0, Stroke::new(1.0, color), StrokeKind::Inside);
    let front = back.translate(vec2(COPY_MARK_OFFSET_PX, COPY_MARK_OFFSET_PX));
    // A 1 px ground-coloured gap separates the front square from the outline behind it.
    painter.rect_filled(front.expand(1.0), 0.0, GROUND);
    painter.rect_filled(front, 0.0, color);
}

fn paint_copied_label(painter: &Painter, right_middle: Pos2) {
    let galley = caps_galley(painter, COPIED_LABEL, INK_ON_SAND);
    let text_left = right_middle.x - galley.size().x;
    painter.galley(
        pos2(text_left, right_middle.y - galley.size().y / 2.0),
        galley,
        INK_ON_SAND,
    );
    paint_square(
        painter,
        pos2(
            text_left - COPIED_SQUARE_TO_TEXT_PX - COPIED_SQUARE_SIDE_PX / 2.0,
            right_middle.y,
        ),
        COPIED_SQUARE_SIDE_PX,
        INK_ON_SAND,
    );
}
