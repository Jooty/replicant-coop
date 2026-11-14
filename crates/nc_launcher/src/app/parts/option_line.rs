use eframe::egui::{
    Align, Align2, Layout, Rect, Response, Sense, Stroke, Ui, UiBuilder, pos2, vec2,
};
use nc_ui_style::{OPTION_LINE_HEIGHT_PX, OPTION_TREE_ARM_END_X_PX, OPTION_TREE_X_PX};

use crate::app::palette::{HAIRLINE_STRONG, HATCH_LINE, INK, INK_UNAVAILABLE};
use crate::app::parts::{ControlLook, on_off_square, paint_hatching};
use crate::app::style::body_font;

pub use nc_ui_style::OptionNesting;

/// One on/off option: its label, a tree line when it is nested, and the on/off square at the right. An
/// unavailable option is hatched and greyed. Clicking anywhere on an editable line flips it.
pub fn option_line(
    ui: &mut Ui,
    label: &str,
    on: &mut bool,
    look: ControlLook,
    nesting: OptionNesting,
) -> Response {
    let sense = if look.editable() {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, mut row) =
        ui.allocate_exact_size(vec2(ui.available_width(), OPTION_LINE_HEIGHT_PX), sense);
    if row.clicked() {
        *on = !*on;
        row.mark_changed();
    }
    let unavailable = look == ControlLook::Unavailable;
    let painter = ui.painter();
    if unavailable {
        paint_hatching(painter, rect, HATCH_LINE);
    }
    paint_tree_lines(ui, rect, nesting);
    let label_left = nesting.label_left_px();
    let ink = if unavailable { INK_UNAVAILABLE } else { INK };
    ui.painter().text(
        pos2(rect.left() + label_left, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        body_font(),
        ink,
    );
    let square = ui
        .scope_builder(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::right_to_left(Align::Center)),
            |ui| on_off_square(ui, on, look),
        )
        .inner;
    if square.changed() {
        row.mark_changed();
    }
    row
}

fn paint_tree_lines(ui: &Ui, rect: Rect, nesting: OptionNesting) {
    let bottom = match nesting {
        OptionNesting::Top => return,
        OptionNesting::Branch => rect.bottom(),
        OptionNesting::LastBranch => rect.center().y,
    };
    let stroke = Stroke::new(1.0, HAIRLINE_STRONG);
    let x = rect.left() + OPTION_TREE_X_PX + 0.5;
    let painter = ui.painter();
    painter.vline(x, rect.top()..=bottom, stroke);
    painter.hline(
        x..=rect.left() + OPTION_TREE_ARM_END_X_PX,
        rect.center().y + 0.5,
        stroke,
    );
}
