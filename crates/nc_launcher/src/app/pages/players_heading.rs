use eframe::egui::{Align2, FontId, Ui, pos2};
use nc_launcher_core::CAMPAIGN_PLAYER_COUNT;

use crate::app::palette::{INK, INK_MUTED};
use crate::app::parts::section_heading;
use crate::app::style::light_family;

pub(super) const PLAYERS_HEADING_HEIGHT_PX: f32 = 32.0;
const PLAYERS_HEADING: &str = "Players";
const COUNT_SIZE_PX: f32 = 22.0;

/// The PLAYERS heading, with the party count at the right only when the seats alone do not tell it.
pub(super) fn players_heading(ui: &mut Ui, party: usize, max_players: u8) {
    let main = ui.max_rect();
    section_heading(ui, PLAYERS_HEADING, None);
    if max_players <= CAMPAIGN_PLAYER_COUNT {
        return;
    }
    let painter = ui.painter();
    let font = FontId::new(COUNT_SIZE_PX, light_family());
    let middle = pos2(
        main.right(),
        main.top() + PLAYERS_HEADING_HEIGHT_PX / 2.0 - 4.0,
    );
    let of = painter.text(
        middle,
        Align2::RIGHT_CENTER,
        format!(" / {max_players}"),
        font.clone(),
        INK_MUTED,
    );
    painter.text(
        pos2(of.left(), middle.y),
        Align2::RIGHT_CENTER,
        party.to_string(),
        font,
        INK,
    );
}
