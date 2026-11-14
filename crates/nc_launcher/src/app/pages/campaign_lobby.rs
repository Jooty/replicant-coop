use eframe::egui::{ScrollArea, Ui, UiBuilder};

use crate::app::motion::page_section;
use crate::app::pages::campaign_card::draw_campaign_card;
use crate::app::pages::lobby_bar::draw_lobby_bar;
use crate::app::pages::lobby_layout::lobby_layout;
use crate::app::pages::lobby_options::draw_lobby_options;
use crate::app::pages::mode_menu::draw_mode_menu;
use crate::app::pages::player_seats::{SeatFacts, draw_player_seats};
use crate::app::pages::save_menu::draw_save_menu;
use crate::app::{Launcher, NetForms};
use crate::net::NetState;

const MODES_TO_OPTIONS_GAP_PX: f32 = 26.0;
pub(super) const OPEN_SEAT_IDLE: &str = "Open";
const OPEN_SEAT_JOINING: &str = "Joining";
const OPEN_SEAT_WAITING: &str = "Waiting";
const LOBBY_BAR_SECTION: usize = 2;

/// What the lobby's left column opens with: the game's modes on New game, the host's saves on Continue, and
/// the host's campaign for a guest on Continue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LobbyMenu {
    Modes,
    Saves,
    Campaign,
}

pub(super) fn draw_campaign_lobby(
    ui: &mut Ui,
    launcher: &mut Launcher,
    forms: &mut NetForms,
    menu: LobbyMenu,
) {
    let layout = lobby_layout(ui.max_rect(), ui.ctx().content_rect());
    ui.scope_builder(UiBuilder::new().max_rect(layout.menu), |ui| {
        ScrollArea::vertical()
            .id_salt("lobby-menu-column")
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                page_section(ui, 0, |ui| match menu {
                    LobbyMenu::Modes => draw_mode_menu(ui, launcher, forms),
                    LobbyMenu::Saves => draw_save_menu(ui, launcher),
                    LobbyMenu::Campaign => draw_campaign_card(ui, launcher),
                });
                ui.add_space(MODES_TO_OPTIONS_GAP_PX);
                page_section(ui, 1, |ui| draw_lobby_options(ui, launcher, forms));
            });
    });
    let state = forms.net.status().state;
    let peer_name = match &state {
        NetState::Connected { peer_name } => Some(peer_name.as_str()),
        _ => None,
    };
    let open_seat_name = match &state {
        NetState::Idle | NetState::Failed(_) => OPEN_SEAT_IDLE,
        NetState::Joining => OPEN_SEAT_JOINING,
        NetState::Hosting { .. } | NetState::Connected { .. } => OPEN_SEAT_WAITING,
    };
    let open_seat_waiting = matches!(state, NetState::Hosting { .. } | NetState::Connected { .. });
    let facts = SeatFacts {
        peer_name,
        open_seat_name,
        open_seat_waiting,
    };
    ui.scope_builder(UiBuilder::new().max_rect(layout.main), |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        draw_player_seats(ui, launcher, facts);
    });
    ui.scope_builder(UiBuilder::new().max_rect(layout.bar), |ui| {
        page_section(ui, LOBBY_BAR_SECTION, |ui| {
            draw_lobby_bar(ui, launcher, forms)
        });
    });
}
