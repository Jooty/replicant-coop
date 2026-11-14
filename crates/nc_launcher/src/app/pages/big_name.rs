use eframe::egui::{Color32, FontId, Frame, Margin, Rect, Stroke, TextEdit, Ui, pos2, vec2};

use crate::app::palette::{INK, INK_MUTED};
use crate::app::style::light_family;

pub(super) const BIG_NAME_HEIGHT_PX: f32 = 36.0;
const BIG_NAME_SIZE_PX: f32 = 30.0;
const BIG_NAME_UNDERLINE_TAIL_PX: f32 = 26.0;
const BIG_NAME_MIN_FIELD_PX: f32 = 60.0;
const BIG_NAME_UNDERLINE_BELOW_PX: f32 = 1.5;

/// A player's name in the large light face, clipped to `rect`.
pub(super) fn paint_big_name(ui: &Ui, rect: Rect, text: &str, ink: Color32) {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), big_name_font(), ink);
    let top = rect.center().y - galley.size().y / 2.0;
    ui.painter()
        .with_clip_rect(rect)
        .galley(pos2(rect.left(), top), galley, ink);
}

/// Your own name, typed into, in the large light face on a muted underline that runs a little past it.
pub(super) fn big_name_field(ui: &mut Ui, rect: Rect, text: &mut String) {
    let text_width = ui
        .painter()
        .layout_no_wrap(text.clone(), big_name_font(), INK)
        .size()
        .x;
    let field_width = (text_width + BIG_NAME_UNDERLINE_TAIL_PX)
        .max(BIG_NAME_MIN_FIELD_PX)
        .min(rect.width());
    let field = Rect::from_min_size(rect.min, vec2(field_width, rect.height()));
    ui.put(
        field,
        TextEdit::singleline(text)
            .frame(Frame::NONE)
            .margin(Margin::ZERO)
            .font(big_name_font())
            .text_color(INK)
            .desired_width(field_width),
    );
    ui.painter().hline(
        rect.left()..=rect.left() + field_width,
        rect.bottom() + BIG_NAME_UNDERLINE_BELOW_PX,
        Stroke::new(1.0, INK_MUTED),
    );
}

fn big_name_font() -> FontId {
    FontId::new(BIG_NAME_SIZE_PX, light_family())
}
