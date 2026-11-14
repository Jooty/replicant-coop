use eframe::egui::{Align2, Color32, Id, Pos2, Rect, Stroke, Ui, pos2, vec2};
use nc_launcher_core::{Easing, STATUS_INVERT_MS};

use crate::app::motion::{changed_since, with_clock};
use crate::app::pages::status_ink::status_ink;
use crate::app::palette::{HAIRLINE, INK, INK_MUTED, INK_UNAVAILABLE, SAND};
use crate::app::parts::caps_galley;
use crate::app::style::body_font;
use crate::lobby::PlayerStatus;

pub(super) const LEDGER_ROW_HEIGHT_PX: f32 = 32.0;
const LEDGER_SIDE_PADDING_PX: f32 = 14.0;
const LOCAL_PING_TEXT: &str = "Local";
const NOTHING_YET_TEXT: &str = "-";
const STATUS_SALT: &str = "seat-status";
const STATUS_INVERT_PADDING_PX: eframe::egui::Vec2 = vec2(6.0, 3.0);

/// What a seat's CHARACTER line shows: a plain label, or a dash while the seat is open.
pub(super) enum CharacterCell {
    Fixed(String),
    Empty,
}

pub(super) enum PingCell {
    Local,
    Millis(u32),
    Unknown,
}

/// The facts under a seat: CHARACTER, STATUS and PING.
pub(super) struct SeatLedger {
    pub character: CharacterCell,
    pub status: Option<PlayerStatus>,
    pub ping: PingCell,
}

pub(super) fn draw_seat_ledger(ui: &mut Ui, rect: Rect, ledger: &SeatLedger, seat_number: usize) {
    let keys = ["Character", "Status", "Ping"];
    for (index, key) in keys.into_iter().enumerate() {
        let row = Rect::from_min_size(
            pos2(
                rect.left(),
                rect.top() + index as f32 * LEDGER_ROW_HEIGHT_PX,
            ),
            vec2(rect.width(), LEDGER_ROW_HEIGHT_PX),
        );
        paint_row_frame(ui, row, index + 1 == keys.len());
        let key_galley = caps_galley(ui.painter(), key, INK_MUTED);
        ui.painter().galley(
            pos2(
                row.left() + LEDGER_SIDE_PADDING_PX,
                row.center().y - key_galley.size().y / 2.0,
            ),
            key_galley,
            INK_MUTED,
        );
        let value_right = pos2(row.right() - LEDGER_SIDE_PADDING_PX, row.center().y);
        match index {
            0 => draw_character(ui, value_right, &ledger.character),
            1 => draw_status(ui, value_right, ledger.status, seat_number),
            _ => draw_ping(ui, value_right, &ledger.ping),
        }
    }
}

fn paint_row_frame(ui: &Ui, row: Rect, last: bool) {
    let stroke = Stroke::new(1.0, HAIRLINE);
    ui.painter().hline(row.x_range(), row.top() + 0.5, stroke);
    if last {
        ui.painter()
            .hline(row.x_range(), row.bottom() - 0.5, stroke);
    }
}

fn draw_character(ui: &Ui, right: Pos2, cell: &CharacterCell) {
    match cell {
        CharacterCell::Fixed(label) => paint_plain(ui, right, label, INK),
        CharacterCell::Empty => paint_plain(ui, right, NOTHING_YET_TEXT, INK_UNAVAILABLE),
    }
}

fn draw_status(ui: &Ui, right: Pos2, status: Option<PlayerStatus>, seat_number: usize) {
    let Some(status) = status else {
        paint_plain(ui, right, NOTHING_YET_TEXT, INK_UNAVAILABLE);
        return;
    };
    let since = changed_since(ui, Id::unique((STATUS_SALT, seat_number)), status.text());
    let flash = with_clock(ui, |clock, _| {
        clock.entrance(since, 0.0, STATUS_INVERT_MS, Easing::Linear)
    });
    if flash >= 1.0 {
        paint_caps(ui, right, status.text(), status_ink(status, false));
        return;
    }
    // A new word shows inverted for an instant, then settles.
    let ink = status_ink(status, true);
    let galley = caps_galley(ui.painter(), status.text(), ink);
    let at = Align2::RIGHT_CENTER.anchor_size(right, galley.size()).min;
    let block = Rect::from_min_size(at, galley.size()).expand2(STATUS_INVERT_PADDING_PX);
    ui.painter().rect_filled(block, 0.0, SAND);
    ui.painter().galley(at, galley, ink);
}

fn draw_ping(ui: &Ui, right: Pos2, ping: &PingCell) {
    match ping {
        PingCell::Local => paint_caps(ui, right, LOCAL_PING_TEXT, INK_MUTED),
        PingCell::Millis(millis) => paint_plain(ui, right, &format!("{millis} ms"), INK),
        PingCell::Unknown => paint_plain(ui, right, NOTHING_YET_TEXT, INK_UNAVAILABLE),
    }
}

fn paint_caps(ui: &Ui, right: Pos2, text: &str, ink: Color32) {
    let galley = caps_galley(ui.painter(), text, ink);
    let at = Align2::RIGHT_CENTER.anchor_size(right, galley.size()).min;
    ui.painter().galley(at, galley, ink);
}

fn paint_plain(ui: &Ui, right: Pos2, text: &str, ink: Color32) {
    ui.painter()
        .text(right, Align2::RIGHT_CENTER, text, body_font(), ink);
}
