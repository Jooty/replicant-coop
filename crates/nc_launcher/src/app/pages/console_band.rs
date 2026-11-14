use eframe::egui::{Align, Align2, FontId, Id, Layout, Rect, Ui, UiBuilder, pos2, vec2};
use nc_launcher_core::{ConsoleLineKind, ConsoleView};

use crate::app::motion::sweep_toward;
use crate::app::palette::{HARD_SHADOW, INK_ON_SAND, NOT_READY, READY};
use crate::app::parts::{LINE_BUTTON_HEIGHT_PX, LineTone, line_button, paint_square, spaced_job};
use crate::app::style::{CAPS_LETTER_SPACING_EM, body_font, caps_font, medium_family};

pub(super) const BAND_HEIGHT_PX: f32 = 44.0;
pub(super) const FAILED_BAND_HEIGHT_PX: f32 = 68.0;
pub(super) const BAND_TO_BACK_PX: f32 = 18.0;
const BAND_SIDE_PADDING_PX: f32 = 18.0;
const BAND_SQUARE_SIDE_PX: f32 = 8.0;
const BAND_SQUARE_TO_TEXT_PX: f32 = 12.0;
const BAND_SHADOW_OFFSET_PX: f32 = 5.0;
const BAND_TIME_FONT_PX: f32 = 12.0;
const FAILED_TITLE_TOP_PX: f32 = 12.0;
const FAILED_ERROR_TOP_PX: f32 = 36.0;
const BAND_SALT: &str = "boot-console-band";
const BACK_LABEL: &str = "Back";

pub(super) fn band_height_if_shown(view: &ConsoleView) -> Option<f32> {
    if view.failed {
        return Some(FAILED_BAND_HEIGHT_PX + BAND_TO_BACK_PX + LINE_BUTTON_HEIGHT_PX);
    }
    view.lines
        .iter()
        .any(|line| line.kind == ConsoleLineKind::Closing && line.word.is_some())
        .then_some(BAND_HEIGHT_PX)
}

pub(super) fn draw_console_band_and_back(
    ui: &mut Ui,
    area: Rect,
    view: &ConsoleView,
    run_salt: u64,
) -> bool {
    let failed = view.failed;
    let height = if failed {
        FAILED_BAND_HEIGHT_PX
    } else {
        BAND_HEIGHT_PX
    };
    let band = Rect::from_min_size(area.min, vec2(area.width(), height));
    let sweep = sweep_toward(ui, Id::unique((BAND_SALT, run_salt)), true);
    let painter = ui.painter().clone();
    let (left, right) = sweep.covered(band.left(), band.right());
    let covered = Rect::from_x_y_ranges(left..=right, band.y_range());
    painter.rect_filled(
        covered.translate(vec2(BAND_SHADOW_OFFSET_PX, BAND_SHADOW_OFFSET_PX)),
        0.0,
        HARD_SHADOW,
    );
    painter.rect_filled(covered, 0.0, if failed { NOT_READY } else { READY });
    let text = painter.with_clip_rect(covered);
    let failed_label = view
        .lines
        .iter()
        .rev()
        .find(|line| line.failed)
        .map(|line| line.label);
    let ended_ms = view
        .lines
        .iter()
        .rev()
        .find_map(|line| line.closed_ms)
        .unwrap_or(0.0);
    let title = match failed_label {
        Some(label) => format!("{label}  failed"),
        None => "Lobby  ready".to_owned(),
    };
    let title_middle = if failed {
        band.top() + FAILED_TITLE_TOP_PX + caps_font().size / 2.0
    } else {
        band.center().y
    };
    let square_x = band.left() + BAND_SIDE_PADDING_PX;
    paint_square(
        &text,
        pos2(square_x + BAND_SQUARE_SIDE_PX / 2.0, title_middle),
        BAND_SQUARE_SIDE_PX,
        INK_ON_SAND,
    );
    let title_galley = text.layout_job(spaced_job(
        &title.to_uppercase(),
        FontId::new(caps_font().size, medium_family()),
        INK_ON_SAND,
        CAPS_LETTER_SPACING_EM,
    ));
    let title_left = square_x + BAND_SQUARE_SIDE_PX + BAND_SQUARE_TO_TEXT_PX;
    text.galley(
        pos2(title_left, title_middle - title_galley.size().y / 2.0),
        title_galley,
        INK_ON_SAND,
    );
    text.text(
        pos2(band.right() - BAND_SIDE_PADDING_PX, title_middle),
        Align2::RIGHT_CENTER,
        format!("{:05.2} S", ended_ms / 1000.0),
        FontId::monospace(BAND_TIME_FONT_PX),
        INK_ON_SAND,
    );
    if !failed {
        return false;
    }
    if let Some(error) = &view.error {
        let width = band.right() - BAND_SIDE_PADDING_PX - title_left;
        let mut job = eframe::egui::text::LayoutJob::simple(
            error.clone(),
            body_font(),
            INK_ON_SAND,
            width.max(0.0),
        );
        job.wrap.max_rows = 1;
        job.wrap.overflow_character = Some('\u{2026}');
        let galley = text.layout_job(job);
        text.galley(
            pos2(title_left, band.top() + FAILED_ERROR_TOP_PX),
            galley,
            INK_ON_SAND,
        );
    }
    let row = Rect::from_min_size(
        pos2(band.left(), band.bottom() + BAND_TO_BACK_PX),
        vec2(band.width(), LINE_BUTTON_HEIGHT_PX),
    );
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(row)
            .layout(Layout::right_to_left(Align::Center)),
        |ui| {
            line_button(
                ui,
                BACK_LABEL,
                LineTone::Plain,
                LINE_BUTTON_HEIGHT_PX,
                None,
                true,
            )
            .clicked()
        },
    )
    .inner
}
