use eframe::egui::{Rect, Sense, Ui, UiBuilder, pos2, vec2};

use crate::app::palette::{HATCH_LINE_DENSE, INK};
use crate::app::parts::{
    ControlLook, LINE_BUTTON_HEIGHT_PX, LineTone, MenuValue, UpdateLineState, caps_label,
    command_button, copy_line, copy_line_id, line_button, menu_row, on_off_square,
    paint_grid_ground, paint_hatching, section_heading, segmented_meter, update_line,
};
use crate::app::style::{body_font, small_font};
use crate::app::{Launcher, Screen};
use crate::config::LauncherConfig;
use crate::home::GameCard;
use crate::net::LinkStatus;
use nc_launcher_core::GameProfile;
use replicant_proto::CampaignCard;

const SHEET_GUTTER_PX: f32 = 24.0;
const SHEET_COLUMN_WIDTH_PX: f32 = 640.0;
const SHEET_GAP_PX: f32 = 14.0;
const SHEET_METER_VALUE: u32 = 50;
const SHEET_METER_MAX: u32 = 100;
const SHEET_METER_CELLS: u32 = 10;
const SHEET_COMMAND_SIZE_PX: (f32, f32) = (176.0, 48.0);
const SHEET_COPY_LINE_WIDTH_PX: f32 = 470.0;
const SHEET_HATCH_HEIGHT_PX: f32 = 34.0;

pub fn fixture_launcher(
    profile: &'static GameProfile,
    cards: Vec<GameCard>,
    link: LinkStatus,
    screen: Screen,
) -> Launcher {
    let mut launcher = Launcher::from_parts(LauncherConfig::default(), None);
    launcher.profile = profile;
    let active = cards
        .iter()
        .find(|card| card.profile.game_id == launcher.profile.game_id);
    launcher.game = active.and_then(|card| card.install.clone());
    launcher.dll_source = active.and_then(|card| card.dll_source.clone());
    launcher.mod_status = active.map_or(launcher.mod_status, |card| card.mod_status);
    launcher.cards = cards;
    launcher.link = link;
    launcher.screen = screen;
    launcher
}

/// Puts the host's campaign card in a guest's launcher, as if the host had just sent it.
pub fn fixture_campaign_card(launcher: &mut Launcher, card: CampaignCard) {
    launcher.hold_campaign_card(card);
}

/// Every shared part of the launcher's look on one page, for the snapshot harness.
pub fn draw_parts_sheet(ui: &mut Ui) {
    let whole = ui.max_rect();
    paint_grid_ground(ui.painter(), whole);
    let column = Rect::from_min_size(
        pos2(
            whole.left() + SHEET_GUTTER_PX,
            whole.top() + SHEET_GUTTER_PX,
        ),
        vec2(
            SHEET_COLUMN_WIDTH_PX,
            whole.height() - 2.0 * SHEET_GUTTER_PX,
        ),
    );
    ui.scope_builder(UiBuilder::new().max_rect(column), |ui| {
        section_heading(ui, "Mode", None);
        ui.add_space(SHEET_GAP_PX);
        menu_row(ui, "Co-op campaign", MenuValue::None, true, true);
        menu_row(ui, "Co-op campaign", MenuValue::None, false, true);
        menu_row(
            ui,
            "Empty",
            MenuValue::Capitals("Unavailable"),
            false,
            false,
        );
        ui.add_space(SHEET_GAP_PX);
        section_heading(ui, "Options", None);
        ui.horizontal(|ui| {
            caps_label(ui, "Names over players", INK);
            on_off_square(ui, &mut true, ControlLook::Editable);
            on_off_square(ui, &mut false, ControlLook::Editable);
            on_off_square(ui, &mut true, ControlLook::ReadOnly);
            on_off_square(ui, &mut false, ControlLook::Unavailable);
        });
        let mut health = SHEET_METER_VALUE;
        segmented_meter(
            ui,
            &mut health,
            SHEET_METER_MAX,
            SHEET_METER_CELLS,
            ControlLook::Editable,
        );
        segmented_meter(
            ui,
            &mut health,
            SHEET_METER_MAX,
            SHEET_METER_CELLS,
            ControlLook::ReadOnly,
        );
        ui.add_space(SHEET_GAP_PX);
        ui.horizontal(|ui| {
            command_button(ui, "Play", SHEET_COMMAND_SIZE_PX, true, true);
            command_button(ui, "Play", SHEET_COMMAND_SIZE_PX, false, true);
            command_button(ui, "Start", SHEET_COMMAND_SIZE_PX, false, false);
        });
        ui.add_space(SHEET_GAP_PX);
        ui.horizontal(|ui| {
            line_button(
                ui,
                "Install the mod",
                LineTone::Warm,
                LINE_BUTTON_HEIGHT_PX,
                None,
                true,
            );
            line_button(
                ui,
                "Uninstall",
                LineTone::Plain,
                LINE_BUTTON_HEIGHT_PX,
                None,
                true,
            );
            line_button(
                ui,
                "Restore",
                LineTone::Plain,
                LINE_BUTTON_HEIGHT_PX,
                None,
                false,
            );
        });
        ui.add_space(SHEET_GAP_PX);
        update_line(
            ui,
            "sheet-update",
            "Build 7b04de2 ready",
            SHEET_COPY_LINE_WIDTH_PX,
            UpdateLineState::Offered,
        );
        ui.add_space(SHEET_GAP_PX);
        copy_line(
            ui,
            "sheet-path",
            "~/.local/share/Steam/steamapps/common/\nNieR Replicant ver.1.22474487139",
            "/fixture/path",
            SHEET_COPY_LINE_WIDTH_PX,
            small_font(),
        );
        ui.add_space(SHEET_GAP_PX);
        let now = ui.input(|input| input.time);
        ui.data_mut(|data| data.insert_temp(copy_line_id("sheet-ticket"), now));
        copy_line(
            ui,
            "sheet-ticket",
            "nc1-q7m2xd-p93fal-w8ltk3-qv7rbn-4hd0se",
            "nc1",
            SHEET_COPY_LINE_WIDTH_PX,
            body_font(),
        );
        ui.add_space(SHEET_GAP_PX);
        let (hatched, _) = ui.allocate_exact_size(
            vec2(SHEET_COPY_LINE_WIDTH_PX, SHEET_HATCH_HEIGHT_PX),
            Sense::hover(),
        );
        paint_hatching(ui.painter(), hatched, HATCH_LINE_DENSE);
    });
}
