use eframe::egui::{Ui, UiBuilder};

use crate::app::pages::campaign_lobby::OPEN_SEAT_IDLE;
use crate::app::pages::lobby_bar::draw_ended_lobby_bar;
use crate::app::pages::lobby_layout::lobby_layout;
use crate::app::pages::player_seats::{SeatFacts, draw_player_seats};
use crate::app::{Launcher, NetForms};

/// A host's lobby that ended from the other side or the network: the seats as they last were, dimmed and still,
/// and the bar with what happened and the way on.
pub(super) fn draw_ended_lobby(ui: &mut Ui, launcher: &mut Launcher, forms: &mut NetForms) {
    let Some(ended) = forms.ended_lobby.clone() else {
        return;
    };
    let layout = lobby_layout(ui.max_rect(), ui.ctx().content_rect());
    // The seats are drawn from the link as it last was; the live one is put back after.
    let live = std::mem::replace(&mut launcher.link, ended.last_seen);
    let peer_name = launcher.link.partner().map(|partner| partner.name.clone());
    ui.scope_builder(UiBuilder::new().max_rect(layout.main), |ui| {
        ui.disable();
        ui.spacing_mut().item_spacing.y = 0.0;
        let facts = SeatFacts {
            peer_name: peer_name.as_deref(),
            open_seat_name: OPEN_SEAT_IDLE,
            open_seat_waiting: false,
        };
        draw_player_seats(ui, launcher, facts);
    });
    launcher.link = live;
    ui.scope_builder(UiBuilder::new().max_rect(layout.bar), |ui| {
        draw_ended_lobby_bar(ui, launcher, forms, &ended.reason);
    });
}
