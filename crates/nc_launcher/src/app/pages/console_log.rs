use std::sync::Arc;

use eframe::egui::{
    Align2, Color32, FontId, Galley, Id, Painter, Rect, Stroke, StrokeKind, Ui, pos2, vec2,
};
use nc_launcher_core::{
    ConsoleLineKind, ConsoleLineView, ConsoleView, HATCH_DRIFT_MS_PER_PX, drift_offset_px,
};

use crate::app::motion::{sweep_toward, with_clock};
use crate::app::palette::{
    HAIRLINE, HAIRLINE_STRONG, HATCH_LINE_DENSE, INK, INK_MUTED, INK_ON_SAND, INK_UNAVAILABLE,
    NOT_READY, READY,
};
use crate::app::parts::{
    HATCH_STEP_ALONG_PX, paint_hatching_shifted, paint_open_square, paint_square,
};

pub(super) const STEP_ROW_PX: f32 = 28.0;
pub(super) const DETAIL_ROW_PX: f32 = 22.0;
pub(super) const GUTTER_WIDTH_PX: f32 = 64.0;
const GUTTER_TO_TEXT_PX: f32 = 34.0;
const STEP_FONT_PX: f32 = 14.0;
const DETAIL_FONT_PX: f32 = 13.0;
const GUTTER_FONT_PX: f32 = 12.0;
const GUTTER_TEXT_INSET_PX: f32 = 12.0;
const STATUS_WIDTH_PX: f32 = 96.0;
const STATUS_HEIGHT_PX: f32 = 18.0;
const STATUS_FONT_PX: f32 = 12.0;
const VALUE_TO_STATUS_PX: f32 = 20.0;
const LEADER_GAP_PX: f32 = 8.0;
const DETAIL_INDENT_PX: f32 = 30.0;
const CONNECTOR_X_PX: f32 = 6.0;
const CONNECTOR_ARM_END_PX: f32 = 20.0;
const CURSOR_GAP_PX: f32 = 2.0;
const CONNECTOR_RISE_PX: f32 = 6.0;
const DETAIL_VALUE_COLUMN_CHARS: usize = 16;
const STEP_MARK_SIDE_PX: f32 = 6.0;
const STEP_MARK_TO_TEXT_PX: f32 = 12.0;
const STEP_TO_DETAILS_GAP_PX: f32 = 2.0;
const LEADER: char = '.';
const ELLIPSIS: char = '\u{2026}';
const STATUS_SALT: &str = "boot-console-status";

struct LineInks {
    label: Color32,
    leaders: Color32,
    value: Color32,
}

pub(super) fn console_log_height(view: &ConsoleView) -> f32 {
    view.lines
        .iter()
        .filter(|line| line.kind != ConsoleLineKind::Closing)
        .map(row_height)
        .sum()
}

pub(super) fn paint_console_log_newest_in_view(
    ui: &Ui,
    area: Rect,
    view: &ConsoleView,
    run_salt: u64,
) {
    let height = console_log_height(view);
    let mut top = area.top() - (height - area.height()).max(0.0);
    let painter = ui.painter().with_clip_rect(area);
    let gutter_line = area.left() + GUTTER_WIDTH_PX + 0.5;
    painter.vline(
        gutter_line,
        area.top()..=area.top() + height.min(area.height()),
        Stroke::new(1.0, HAIRLINE),
    );
    let text_left = area.left() + GUTTER_WIDTH_PX + GUTTER_TO_TEXT_PX;
    let status_rect_left = area.right() - STATUS_WIDTH_PX;
    for (index, line) in view.lines.iter().enumerate() {
        if line.kind == ConsoleLineKind::Closing {
            continue;
        }
        let row = Rect::from_min_max(
            pos2(area.left(), top),
            pos2(area.right(), top + row_height(line)),
        );
        top = row.bottom();
        paint_gutter(&painter, row, line);
        let cursor = view.cursor_line == Some(index) && line.typed_share < 1.0;
        match line.kind {
            ConsoleLineKind::Step => {
                let value_right = status_rect_left - VALUE_TO_STATUS_PX;
                let inks = LineInks {
                    label: INK,
                    leaders: INK_UNAVAILABLE,
                    value: INK,
                };
                let middle = row.top() + STEP_ROW_PX / 2.0;
                paint_step_mark(&painter, text_left, middle, line);
                paint_typed_line(
                    &painter,
                    line,
                    text_left,
                    value_right,
                    middle,
                    FontId::monospace(STEP_FONT_PX),
                    &inks,
                    cursor,
                );
                let slot = Rect::from_center_size(
                    pos2(status_rect_left + STATUS_WIDTH_PX / 2.0, middle),
                    vec2(STATUS_WIDTH_PX, STATUS_HEIGHT_PX),
                );
                paint_status(ui, &painter, slot, line, (run_salt, index));
            }
            ConsoleLineKind::Detail { last } => {
                let connector_x = text_left + CONNECTOR_X_PX + 0.5;
                let middle = row.center().y;
                let stroke = Stroke::new(1.0, HAIRLINE_STRONG);
                let bottom = if last { middle } else { row.bottom() };
                painter.vline(connector_x, row.top() - CONNECTOR_RISE_PX..=bottom, stroke);
                painter.hline(
                    connector_x..=text_left + CONNECTOR_ARM_END_PX,
                    middle + 0.5,
                    stroke,
                );
                paint_detail_line(&painter, line, text_left + DETAIL_INDENT_PX, middle, cursor);
            }
            ConsoleLineKind::Closing => {}
        }
    }
}

fn row_height(line: &ConsoleLineView) -> f32 {
    match line.kind {
        ConsoleLineKind::Step => STEP_ROW_PX + STEP_TO_DETAILS_GAP_PX,
        ConsoleLineKind::Detail { .. } => DETAIL_ROW_PX,
        ConsoleLineKind::Closing => 0.0,
    }
}

fn paint_gutter(painter: &Painter, row: Rect, line: &ConsoleLineView) {
    let middle = match line.kind {
        ConsoleLineKind::Step => row.top() + STEP_ROW_PX / 2.0,
        _ => row.center().y,
    };
    let ink = match line.kind {
        ConsoleLineKind::Step => INK_MUTED,
        _ => INK_UNAVAILABLE,
    };
    painter.text(
        pos2(row.left() + GUTTER_WIDTH_PX - GUTTER_TEXT_INSET_PX, middle),
        Align2::RIGHT_CENTER,
        seconds_text(line.began_ms),
        FontId::monospace(GUTTER_FONT_PX),
        ink,
    );
}

fn paint_typed_line(
    painter: &Painter,
    line: &ConsoleLineView,
    left: f32,
    value_right: f32,
    middle: f32,
    font: FontId,
    inks: &LineInks,
    cursor: bool,
) {
    let glyph = |text: String, color: Color32| -> Arc<Galley> {
        painter.layout_no_wrap(text, font.clone(), color)
    };
    let char_width = glyph(LEADER.to_string(), inks.leaders).size().x.max(1.0);
    let label: Vec<char> = line.label.chars().collect();
    let label_width = char_width * label.len() as f32;
    let room_for_value =
        (value_right - left - label_width - 2.0 * LEADER_GAP_PX - char_width * 3.0).max(char_width);
    let value: Vec<char> = line
        .value
        .as_deref()
        .map(|value| elided_from_the_start(value, (room_for_value / char_width) as usize))
        .unwrap_or_default();
    let value_width = char_width * value.len() as f32;
    let leaders_left = left + label_width + LEADER_GAP_PX;
    let leaders_right = if value.is_empty() {
        value_right
    } else {
        value_right - value_width - LEADER_GAP_PX
    };
    let leaders = ((leaders_right - leaders_left) / char_width)
        .floor()
        .max(0.0) as usize;
    let total = label.len() + leaders + value.len();
    let typed = (line.typed_share * total as f32).floor() as usize;
    let label_typed = typed.min(label.len());
    let leaders_typed = typed.saturating_sub(label.len()).min(leaders);
    let value_typed = typed.saturating_sub(label.len() + leaders).min(value.len());
    let top_of = |galley: &Arc<Galley>| middle - galley.size().y / 2.0;
    let label_galley = glyph(label[..label_typed].iter().collect(), inks.label);
    let mut cursor_x = left + label_galley.size().x;
    painter.galley(pos2(left, top_of(&label_galley)), label_galley, inks.label);
    if leaders_typed > 0 {
        let galley = glyph(
            std::iter::repeat_n(LEADER, leaders_typed).collect(),
            inks.leaders,
        );
        cursor_x = leaders_left + galley.size().x;
        painter.galley(pos2(leaders_left, top_of(&galley)), galley, inks.leaders);
    }
    if value_typed > 0 {
        let value_left = value_right - value_width;
        let galley = glyph(value[..value_typed].iter().collect(), inks.value);
        cursor_x = value_left + galley.size().x;
        painter.galley(pos2(value_left, top_of(&galley)), galley, inks.value);
    }
    if cursor {
        let block = glyph("M".to_owned(), INK);
        let rect =
            Rect::from_min_size(pos2(cursor_x + CURSOR_GAP_PX, top_of(&block)), block.size());
        painter.rect_filled(rect, 0.0, INK);
    }
}

fn paint_step_mark(painter: &Painter, text_left: f32, middle: f32, line: &ConsoleLineView) {
    let center = pos2(
        text_left - STEP_MARK_TO_TEXT_PX - STEP_MARK_SIDE_PX / 2.0,
        middle,
    );
    match (line.word, line.failed) {
        (Some(_), true) => paint_square(painter, center, STEP_MARK_SIDE_PX, NOT_READY),
        (Some(_), false) => paint_square(painter, center, STEP_MARK_SIDE_PX, INK),
        (None, _) => paint_open_square(painter, center, STEP_MARK_SIDE_PX, INK_MUTED),
    }
}

fn paint_detail_line(
    painter: &Painter,
    line: &ConsoleLineView,
    left: f32,
    middle: f32,
    cursor: bool,
) {
    let font = FontId::monospace(DETAIL_FONT_PX);
    let char_width = painter
        .layout_no_wrap(LEADER.to_string(), font.clone(), INK_MUTED)
        .size()
        .x;
    let label: Vec<char> = line.label.chars().collect();
    let value: Vec<char> = line.value.as_deref().unwrap_or_default().chars().collect();
    let typed = (line.typed_share * (label.len() + value.len()) as f32).floor() as usize;
    let label_typed = typed.min(label.len());
    let value_typed = typed.saturating_sub(label.len()).min(value.len());
    let label_galley = painter.layout_no_wrap(
        label[..label_typed].iter().collect(),
        font.clone(),
        INK_UNAVAILABLE,
    );
    let top = middle - label_galley.size().y / 2.0;
    let mut cursor_x = left + label_galley.size().x;
    painter.galley(pos2(left, top), label_galley, INK_UNAVAILABLE);
    if value_typed > 0 {
        let value_left = left + char_width * DETAIL_VALUE_COLUMN_CHARS as f32;
        let galley = painter.layout_no_wrap(
            value[..value_typed].iter().collect(),
            font.clone(),
            INK_MUTED,
        );
        cursor_x = value_left + galley.size().x;
        painter.galley(pos2(value_left, top), galley, INK_MUTED);
    }
    if cursor {
        let block = painter.layout_no_wrap("M".to_owned(), font, INK);
        let rect = Rect::from_min_size(pos2(cursor_x + CURSOR_GAP_PX, top), block.size());
        painter.rect_filled(rect, 0.0, INK);
    }
}

fn elided_from_the_start(value: &str, room: usize) -> Vec<char> {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= room || room < 2 {
        return chars;
    }
    // A path keeps its end, where the game's own folder is.
    let mut kept = vec![ELLIPSIS];
    kept.extend_from_slice(&chars[chars.len() - (room - 1)..]);
    kept
}

fn paint_status(
    ui: &Ui,
    painter: &Painter,
    slot: Rect,
    line: &ConsoleLineView,
    salt: (u64, usize),
) {
    let sweep = sweep_toward(ui, Id::unique((STATUS_SALT, salt)), line.word.is_some());
    if line.waiting {
        let focused = ui.input(|input| input.focused);
        let shift = with_clock(ui, |clock, _| {
            if focused {
                clock.drift_px(HATCH_DRIFT_MS_PER_PX, HATCH_STEP_ALONG_PX)
            } else if clock.motion() {
                // Unfocused, it moves only when the window repaints anyway.
                drift_offset_px(clock.now(), HATCH_DRIFT_MS_PER_PX, HATCH_STEP_ALONG_PX)
            } else {
                0.0
            }
        });
        paint_hatching_shifted(painter, slot, HATCH_LINE_DENSE, shift);
        painter.rect_stroke(
            slot,
            0.0,
            Stroke::new(1.0, HAIRLINE_STRONG),
            StrokeKind::Inside,
        );
        if let Some(waited_ms) = line.waited_ms {
            painter.text(
                slot.center(),
                Align2::CENTER_CENTER,
                seconds_text(waited_ms),
                FontId::monospace(STATUS_FONT_PX),
                INK_MUTED,
            );
        }
        return;
    }
    let Some(word) = line.word else {
        return;
    };
    let fill = if line.failed { NOT_READY } else { READY };
    let (left, right) = sweep.covered(slot.left(), slot.right());
    let covered = Rect::from_x_y_ranges(left..=right, slot.y_range());
    painter.rect_filled(covered, 0.0, fill);
    let text = painter.with_clip_rect(covered.intersect(painter.clip_rect()));
    text.text(
        slot.center(),
        Align2::CENTER_CENTER,
        word,
        FontId::monospace(STATUS_FONT_PX),
        INK_ON_SAND,
    );
}

fn seconds_text(ms: f32) -> String {
    format!("{:05.2}", ms / 1000.0)
}
