use eframe::egui::{Align2, Rect, Response, Sense, Ui, pos2, vec2};
use nc_ui_style::{
    MENU_ROW_HEIGHT_PX, MENU_ROW_LEFT_PADDING_PX, MENU_ROW_RIGHT_PADDING_PX,
    MENU_ROW_SHADOW_OFFSET_PX, MENU_ROW_SQUARE_CENTRE_X_PX, MENU_ROW_SQUARE_SIDE_PX,
};

use crate::app::motion::sweep_toward;
use crate::app::palette::{HATCH_LINE, HOVER_WASH, INK, INK_ON_SAND, INK_UNAVAILABLE};
use crate::app::parts::{
    caps_galley, paint_hatching, paint_in_two_inks, paint_square, paint_sweep_fill,
};
use crate::app::style::body_font;

const SELECTION_SWEEP_SALT: &str = "menu-row-sweep";

/// What a menu row's value at the right reads like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuValue<'a> {
    None,
    Plain(&'a str),
    Capitals(&'a str),
}

/// One line of a menu (modes, saves). The selected row is a sand bar with dark ink and a hard offset shadow,
/// swept in from the left when picked and away from the left when left; an unavailable row is hatched and
/// greyed and cannot be picked.
pub fn menu_row(
    ui: &mut Ui,
    label: &str,
    value: MenuValue,
    selected: bool,
    available: bool,
) -> Response {
    let sense = if available {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), MENU_ROW_HEIGHT_PX), sense);
    let sweep = sweep_toward(
        ui,
        response.id.with(SELECTION_SWEEP_SALT),
        selected && available,
    );
    let painter = ui.painter();
    let covered = if available {
        if response.hovered() && !sweep.is_full() {
            painter.rect_filled(rect, 0.0, HOVER_WASH);
        }
        paint_sweep_fill(painter, rect, sweep, MENU_ROW_SHADOW_OFFSET_PX)
    } else {
        paint_hatching(painter, rect, HATCH_LINE);
        Rect::from_x_y_ranges(rect.left()..=rect.left(), rect.y_range())
    };
    paint_in_two_inks(painter, rect, covered, |painter, on_sand| {
        let ink = match (available, on_sand) {
            (false, _) => INK_UNAVAILABLE,
            (true, true) => INK_ON_SAND,
            (true, false) => INK,
        };
        if on_sand {
            paint_square(
                painter,
                pos2(rect.left() + MENU_ROW_SQUARE_CENTRE_X_PX, rect.center().y),
                MENU_ROW_SQUARE_SIDE_PX,
                INK_ON_SAND,
            );
        }
        painter.text(
            pos2(rect.left() + MENU_ROW_LEFT_PADDING_PX, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            body_font(),
            ink,
        );
        let right = pos2(rect.right() - MENU_ROW_RIGHT_PADDING_PX, rect.center().y);
        match value {
            MenuValue::None => {}
            MenuValue::Plain(text) => {
                painter.text(right, Align2::RIGHT_CENTER, text, body_font(), ink);
            }
            MenuValue::Capitals(text) => {
                let galley = caps_galley(painter, text, ink);
                let at = Align2::RIGHT_CENTER.anchor_size(right, galley.size()).min;
                painter.galley(at, galley, ink);
            }
        }
    });
    response
}
