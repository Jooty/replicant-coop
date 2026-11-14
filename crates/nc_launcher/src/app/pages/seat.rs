use eframe::egui::{Rect, TextureHandle, Ui, pos2, vec2};
use nc_launcher_core::SeatArt;

use crate::app::pages::big_name::{BIG_NAME_HEIGHT_PX, big_name_field, paint_big_name};
use crate::app::pages::seat_art::paint_seat_art;
use crate::app::pages::seat_ledger::{LEDGER_ROW_HEIGHT_PX, SeatLedger, draw_seat_ledger};
use crate::app::palette::{HATCH_LINE, INK, INK_MUTED, INK_UNAVAILABLE};
use crate::app::parts::{caps_galley, paint_hatching_shifted, paint_square};

const LEDGER_ROWS: f32 = 3.0;
const SEAT_ART_TO_LEDGER_PX: f32 = 8.0;
const SEAT_INSET_PX: f32 = 14.0;
const SEAT_OVERLAY_FROM_ART_BOTTOM_PX: f32 = 78.0;
const SEAT_META_GAP_PX: f32 = 14.0;
const SEAT_NAME_TOP_PX: f32 = 24.0;
const YOU_SQUARE_SIDE_PX: f32 = 7.0;
const YOU_SQUARE_TO_TEXT_PX: f32 = 9.0;
const YOU_LABEL: &str = "You";

/// Whose name a seat shows: a fixed one, your own while it may be typed into (an underlined field), or
/// nobody's yet.
pub(super) enum SeatName<'a> {
    Fixed(&'a str),
    Typed(&'a mut String),
    Open(&'static str),
}

/// One campaign seat: the art fading down into the seat number, role and name, and the ledger under it.
pub(super) struct Seat<'a> {
    pub number: usize,
    pub role: &'static str,
    pub you: bool,
    pub name: SeatName<'a>,
    pub ledger: SeatLedger,
    pub art_shown: f32,
    pub hatch_shift_px: f32,
}

pub(super) fn seat_height_needed(art_height: f32) -> f32 {
    art_height + SEAT_ART_TO_LEDGER_PX + LEDGER_ROWS * LEDGER_ROW_HEIGHT_PX
}

pub(super) fn seat_art_height(seat: Rect) -> f32 {
    seat.width()
        .min(seat.height() - seat_height_needed(0.0))
        .max(0.0)
}

pub(super) fn draw_seat(
    ui: &mut Ui,
    rect: Rect,
    seat: Seat,
    art: Option<&TextureHandle>,
    crop: SeatArt,
) {
    let art_rect = Rect::from_min_size(rect.min, vec2(rect.width(), seat_art_height(rect)));
    if matches!(seat.name, SeatName::Open(_)) {
        paint_hatching_shifted(ui.painter(), art_rect, HATCH_LINE, seat.hatch_shift_px);
    } else {
        paint_seat_art(ui.painter(), art_rect, art, crop, seat.art_shown);
    }
    let over_top = art_rect.bottom() - SEAT_OVERLAY_FROM_ART_BOTTOM_PX;
    let inner_left = rect.left() + SEAT_INSET_PX;
    let inner_right = rect.right() - SEAT_INSET_PX;
    paint_meta(ui, &seat, over_top, inner_left, inner_right);
    let name_rect = Rect::from_min_max(
        pos2(inner_left, over_top + SEAT_NAME_TOP_PX),
        pos2(
            inner_right,
            over_top + SEAT_NAME_TOP_PX + BIG_NAME_HEIGHT_PX,
        ),
    );
    draw_name(ui, name_rect, seat.name);
    let ledger_rect = Rect::from_min_max(
        pos2(rect.left(), art_rect.bottom() + SEAT_ART_TO_LEDGER_PX),
        rect.right_bottom(),
    );
    draw_seat_ledger(ui, ledger_rect, &seat.ledger, seat.number);
}

fn paint_meta(ui: &Ui, seat: &Seat, top: f32, left: f32, right: f32) {
    let painter = ui.painter();
    let number = caps_galley(painter, &format!("{:02}", seat.number), INK_MUTED);
    let middle = top + number.size().y / 2.0;
    let role_left = left + number.size().x + SEAT_META_GAP_PX;
    painter.galley(pos2(left, top), number, INK_MUTED);
    let role = caps_galley(painter, seat.role, INK_MUTED);
    painter.galley(pos2(role_left, top), role, INK_MUTED);
    if seat.you {
        let you = caps_galley(painter, YOU_LABEL, INK);
        let you_left = right - you.size().x;
        painter.galley(pos2(you_left, top), you, INK);
        paint_square(
            painter,
            pos2(
                you_left - YOU_SQUARE_TO_TEXT_PX - YOU_SQUARE_SIDE_PX / 2.0,
                middle,
            ),
            YOU_SQUARE_SIDE_PX,
            INK,
        );
    }
}

fn draw_name(ui: &mut Ui, rect: Rect, name: SeatName) {
    match name {
        SeatName::Fixed(text) => paint_big_name(ui, rect, text, INK),
        SeatName::Open(text) => paint_big_name(ui, rect, text, INK_UNAVAILABLE),
        SeatName::Typed(text) => big_name_field(ui, rect, text),
    }
}
