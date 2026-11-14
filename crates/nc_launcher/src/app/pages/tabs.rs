use eframe::egui::{self, Frame, Margin, Rect, Ui, UiBuilder, pos2};

use nc_launcher_core::{AfterSessionEnd, after_session_end, session_end_log_line};

use crate::app::motion::{mark_page, set_motion};
use crate::app::pages::boot_console::draw_boot_console;
use crate::app::pages::continue_game::draw_continue;
use crate::app::pages::delete_saves_menu::draw_delete_saves_menu;
use crate::app::pages::ended_lobby::draw_ended_lobby;
use crate::app::pages::home::draw_home;
use crate::app::pages::lobby::session_open;
use crate::app::pages::logs::draw_logs;
use crate::app::pages::new_game::draw_new_game;
use crate::app::pages::settings::draw_settings;
use crate::app::pages::ticket_dialog::draw_ticket_dialog;
use crate::app::pages::top_bar::{TOP_BAR_HEIGHT_PX, draw_home_bar, draw_logs_bar, draw_top_bar};
use crate::app::palette::GROUND;
use crate::app::parts::{DOUBLE_RULE_HEIGHT_PX, paint_double_rule, paint_grid_ground};
use crate::app::{
    EndedLobby, Launcher, LobbyFailure, NetForms, RESTART_TAB_LOGS, RESTART_TAB_SETTINGS, Screen,
};
use crate::net::LOBBY_MODE_CONTINUE;

pub(super) const SIDE_GUTTER_PX: f32 = 24.0;
const PAGE_TOP_GAP_PX: f32 = 16.0;
const PAGE_BOTTOM_GAP_PX: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    NewGame,
    Continue,
    Settings,
}

impl Tab {
    pub const ALL: [(Tab, &'static str); 3] = [
        (Tab::NewGame, "New game"),
        (Tab::Continue, "Continue"),
        (Tab::Settings, "Settings"),
    ];

    pub const WITHOUT_A_LOBBY: [(Tab, &'static str); 1] = [(Tab::Settings, "Settings")];

    /// The tabs the game page shows: all three in a lobby, only Settings without one. Logs are the
    /// launcher's, reached from home.
    pub fn shown(lobby_open: bool) -> &'static [(Tab, &'static str)] {
        if lobby_open {
            &Tab::ALL
        } else {
            &Tab::WITHOUT_A_LOBBY
        }
    }

    pub fn from_restart_value(value: Option<&str>) -> Tab {
        match value {
            Some(RESTART_TAB_SETTINGS) => Tab::Settings,
            _ => Tab::NewGame,
        }
    }

    /// The screen a restarted launcher opens on: the launcher's Logs when asked for them, else the game page.
    pub fn restart_screen(value: Option<&str>) -> Screen {
        match value {
            Some(RESTART_TAB_LOGS) => Screen::Logs,
            _ => Screen::Game,
        }
    }
}

pub fn draw(ui: &mut Ui, tab: &mut Tab, launcher: &mut Launcher, forms: &mut NetForms) {
    set_motion(ui.ctx(), launcher.config.motion);
    follow_the_session_ends(*tab, launcher, forms);
    if forms.console.is_some() {
        draw_boot_console(ui, tab, launcher, forms);
        return;
    }
    follow_the_lobby_tab(tab, launcher);
    let lobby_open = session_open(forms);
    if lobby_open || launcher.screen != Screen::Game {
        forms.ended_lobby = None;
    }
    let ended = forms.ended_lobby.is_some() && on_a_lobby_tab(*tab);
    if !lobby_open && !ended && on_a_lobby_tab(*tab) && launcher.screen == Screen::Game {
        launcher.screen = Screen::Home;
    }
    // An ended lobby is the same page as the live one it was, so nothing on it enters again.
    let page = (
        launcher.screen,
        *tab,
        launcher.profile.game_id,
        lobby_open || ended,
    );
    mark_page(ui, page);
    egui::Panel::top("top-bar")
        .exact_size(TOP_BAR_HEIGHT_PX)
        .frame(bar_frame())
        .show_separator_line(false)
        .show(ui, |ui| match launcher.screen {
            Screen::Home => draw_home_bar(ui, launcher),
            Screen::Game if ended => draw_top_bar(ui, tab, launcher, &[]),
            Screen::Game => draw_top_bar(ui, tab, launcher, Tab::shown(lobby_open)),
            Screen::Logs => draw_logs_bar(ui, launcher),
        });
    egui::CentralPanel::default()
        .frame(Frame::NONE.fill(GROUND))
        .show(ui, |ui| {
            let whole = ui.max_rect();
            let painter = ui.painter();
            paint_double_rule(
                painter,
                whole.left() + SIDE_GUTTER_PX..=whole.right() - SIDE_GUTTER_PX,
                whole.top(),
            );
            let ground = Rect::from_min_max(
                pos2(whole.left(), whole.top() + DOUBLE_RULE_HEIGHT_PX),
                whole.right_bottom(),
            );
            paint_grid_ground(painter, ground);
            let page = Rect::from_min_max(
                pos2(
                    ground.left() + SIDE_GUTTER_PX,
                    ground.top() + PAGE_TOP_GAP_PX,
                ),
                pos2(
                    ground.right() - SIDE_GUTTER_PX,
                    ground.bottom() - PAGE_BOTTOM_GAP_PX,
                ),
            );
            ui.scope_builder(UiBuilder::new().max_rect(page), |ui| {
                match (launcher.screen, *tab) {
                    (Screen::Home, _) => draw_home(ui, tab, launcher, forms),
                    (Screen::Game, _) if ended => draw_ended_lobby(ui, launcher, forms),
                    (Screen::Game, Tab::NewGame) => draw_new_game(ui, launcher, forms),
                    (Screen::Game, Tab::Continue) => draw_continue(ui, launcher, forms),
                    (Screen::Game, Tab::Settings) => draw_settings(ui, launcher, forms),
                    (Screen::Logs, _) => draw_logs(ui, launcher),
                }
            });
        });
    if launcher.screen == Screen::Home && forms.join_dialog.is_some() {
        draw_ticket_dialog(ui, launcher, forms);
    }
    if launcher.screen == Screen::Home {
        draw_delete_saves_menu(ui, launcher, forms);
    } else {
        launcher.coop_save_menu = None;
    }
}

pub(super) fn lobby_tab(kind: u8) -> Tab {
    if kind == LOBBY_MODE_CONTINUE {
        Tab::Continue
    } else {
        Tab::NewGame
    }
}

fn on_a_lobby_tab(tab: Tab) -> bool {
    matches!(tab, Tab::NewGame | Tab::Continue)
}

fn follow_the_session_ends(tab: Tab, launcher: &mut Launcher, forms: &mut NetForms) {
    let on_lobby_page =
        forms.console.is_none() && launcher.screen == Screen::Game && on_a_lobby_tab(tab);
    for (cause, last_seen) in std::mem::take(&mut launcher.session_ends) {
        let after = after_session_end(cause, on_lobby_page, last_seen.guest);
        launcher.log.push(session_end_log_line(cause, after));
        let reason = cause.reason_line().map(str::to_owned);
        if let Some(reason) = reason.clone() {
            forms.lobby_failure = Some(LobbyFailure {
                game_id: launcher.profile.game_id,
                reason,
            });
        }
        match after {
            AfterSessionEnd::Home => launcher.screen = Screen::Home,
            AfterSessionEnd::StayPut => {}
            AfterSessionEnd::EndedLobby => {
                forms.ended_lobby = Some(EndedLobby {
                    reason: reason.unwrap_or_default(),
                    last_seen,
                });
            }
        }
    }
}

fn follow_the_lobby_tab(tab: &mut Tab, launcher: &mut Launcher) {
    if !matches!(*tab, Tab::NewGame | Tab::Continue) {
        return;
    }
    let Some(kind) = launcher.tab_to_follow.take() else {
        return;
    };
    *tab = lobby_tab(kind);
}

fn bar_frame() -> Frame {
    Frame::NONE
        .fill(GROUND)
        .inner_margin(Margin::symmetric(SIDE_GUTTER_PX as i8, 0))
}
