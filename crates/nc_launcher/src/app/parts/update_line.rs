use eframe::egui::{Id, Response, Sense, Stroke, Ui, pos2, vec2};
use nc_launcher_core::{HATCH_DRIFT_MS_PER_PX, drift_offset_px};

use crate::app::motion::{sweep_toward, with_clock};
use crate::app::palette::{
    HAIRLINE_STRONG, HATCH_LINE, INK_MUTED, INK_ON_SAND, INK_UNAVAILABLE, NOT_READY,
};
use crate::app::parts::{
    HATCH_STEP_ALONG_PX, paint_hatching_shifted, paint_in_two_inks, paint_sweep_fill,
    paint_triangle_right,
};
use crate::app::style::body_font;

const UPDATE_LINE_SIDE_PADDING_PX: f32 = 12.0;
pub const UPDATE_LINE_TOP_PADDING_PX: f32 = 6.0;
const UPDATE_LINE_BOTTOM_PADDING_PX: f32 = 5.0;
const UPDATE_MARK_RIGHT_INSET_PX: f32 = 2.0;
const UPDATE_TEXT_TO_MARK_PX: f32 = 14.0;
const UPDATE_SHADOW_OFFSET_PX: f32 = 4.0;
const UPDATE_SWEEP_SALT: &str = "update-line-sweep";

/// Where an offered update stands: waiting for a click, under way, or offered but not now (another task runs
/// or the game cannot be switched to).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateLineState {
    Offered,
    Running,
    Held,
}

/// A line of text that is its own update control: warm ink over a thin underline with a small mark at the
/// right at rest; under the pointer the sand fill sweeps over it. While the update runs it shows `text` over
/// drifting hatching and takes no clicks.
pub fn update_line(
    ui: &mut Ui,
    id_salt: &str,
    text: &str,
    width: f32,
    state: UpdateLineState,
) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), body_font(), NOT_READY);
    let height = galley.size().y + UPDATE_LINE_TOP_PADDING_PX + UPDATE_LINE_BOTTOM_PADDING_PX;
    let sense = if state == UpdateLineState::Offered {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), sense);
    let hovered = response.hovered() && state == UpdateLineState::Offered;
    let sweep = sweep_toward(ui, Id::unique((UPDATE_SWEEP_SALT, id_salt)), hovered);
    let painter = ui.painter();
    let text_at = pos2(
        rect.left() + UPDATE_LINE_SIDE_PADDING_PX,
        rect.top() + UPDATE_LINE_TOP_PADDING_PX,
    );
    let mark_tip = pos2(
        rect.right() - UPDATE_LINE_SIDE_PADDING_PX - UPDATE_MARK_RIGHT_INSET_PX,
        rect.center().y,
    );
    if state == UpdateLineState::Running {
        paint_hatching_shifted(painter, rect, HATCH_LINE, running_drift(ui));
        painter.galley_with_override_text_color(text_at, galley, INK_MUTED);
        return response;
    }
    let rest_ink = match state {
        UpdateLineState::Offered => NOT_READY,
        _ => INK_UNAVAILABLE,
    };
    if !sweep.is_full() {
        painter.hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            Stroke::new(1.0, HAIRLINE_STRONG),
        );
    }
    let covered = paint_sweep_fill(painter, rect, sweep, UPDATE_SHADOW_OFFSET_PX);
    paint_in_two_inks(painter, rect, covered, |painter, on_sand| {
        let ink = if on_sand { INK_ON_SAND } else { rest_ink };
        painter.galley_with_override_text_color(text_at, galley.clone(), ink);
        if state == UpdateLineState::Offered {
            paint_triangle_right(painter, mark_tip, ink);
        }
    });
    response
}

/// The width an update line needs to show `text` and its mark on one row.
pub fn update_line_width(ui: &Ui, text: &str) -> f32 {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), body_font(), NOT_READY);
    galley.size().x + 2.0 * UPDATE_LINE_SIDE_PADDING_PX + UPDATE_TEXT_TO_MARK_PX
}

fn running_drift(ui: &Ui) -> f32 {
    let focused = ui.input(|input| input.focused);
    with_clock(ui, |clock, _| {
        if focused {
            clock.drift_px(HATCH_DRIFT_MS_PER_PX, HATCH_STEP_ALONG_PX)
        } else if clock.motion() {
            // Unfocused, it moves only when the window repaints anyway.
            drift_offset_px(clock.now(), HATCH_DRIFT_MS_PER_PX, HATCH_STEP_ALONG_PX)
        } else {
            0.0
        }
    })
}
