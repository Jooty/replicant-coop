use eframe::egui::{FontId, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::app::motion::sweep_toward;
use crate::app::palette::{HATCH_LINE, INK, INK_MUTED, INK_ON_SAND, INK_UNAVAILABLE};
use crate::app::parts::{
    paint_hatching, paint_in_two_inks, paint_square, paint_sweep_fill, spaced_job,
};
use crate::app::style::medium_family;

const COMMAND_LABEL_SIZE_PX: f32 = 15.0;
const COMMAND_LETTER_SPACING_EM: f32 = 0.32;
const COMMAND_LEFT_PADDING_PX: f32 = 22.0;
const COMMAND_SQUARE_SIDE_PX: f32 = 9.0;
const COMMAND_SQUARE_TO_TEXT_PX: f32 = 14.0;
const COMMAND_SHADOW_OFFSET_PX: f32 = 5.0;
const SELECTION_SWEEP_SALT: &str = "command-sweep";

/// A main command such as PLAY or START: a sand block with dark ink and a hard offset shadow when `inverted`,
/// an outline otherwise. Hovering inverts it, the fill sweeping in from the left; a disabled one is hatched
/// and greyed.
pub fn command_button(
    ui: &mut Ui,
    label: &str,
    size: (f32, f32),
    inverted: bool,
    enabled: bool,
) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(size.0, size.1), sense);
    let filled = enabled && (inverted || response.hovered());
    let sweep = sweep_toward(ui, response.id.with(SELECTION_SWEEP_SALT), filled);
    let painter = ui.painter();
    if !sweep.is_full() {
        if !enabled {
            paint_hatching(painter, rect, HATCH_LINE);
        }
        let border = if enabled { INK_MUTED } else { INK_UNAVAILABLE };
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, border), StrokeKind::Inside);
    }
    let covered = paint_sweep_fill(painter, rect, sweep, COMMAND_SHADOW_OFFSET_PX);
    let square_x = rect.left() + COMMAND_LEFT_PADDING_PX + COMMAND_SQUARE_SIDE_PX / 2.0;
    let text_left =
        rect.left() + COMMAND_LEFT_PADDING_PX + COMMAND_SQUARE_SIDE_PX + COMMAND_SQUARE_TO_TEXT_PX;
    paint_in_two_inks(painter, rect, covered, |painter, on_sand| {
        let ink = if on_sand {
            INK_ON_SAND
        } else if enabled {
            INK
        } else {
            INK_UNAVAILABLE
        };
        paint_square(
            painter,
            pos2(square_x, rect.center().y),
            COMMAND_SQUARE_SIDE_PX,
            ink,
        );
        let galley = painter.layout_job(spaced_job(
            &label.to_uppercase(),
            FontId::new(COMMAND_LABEL_SIZE_PX, medium_family()),
            ink,
            COMMAND_LETTER_SPACING_EM,
        ));
        painter.galley(
            pos2(text_left, rect.center().y - galley.size().y / 2.0),
            galley,
            ink,
        );
    });
    response
}
