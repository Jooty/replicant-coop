use eframe::egui::{Rect, ScrollArea, Ui, UiBuilder, pos2};

use crate::app::motion::page_section;
use crate::app::pages::settings_game::draw_game_section;
use crate::app::pages::settings_in_game::draw_in_game_section;
use crate::app::pages::settings_saves::draw_saves_section;
use crate::app::pages::settings_updates::draw_updates_section;
use crate::app::pages::settings_window::draw_window_section;
use crate::app::{Launcher, NetForms};

const SETTINGS_LEFT_SHARE: f32 = 532.0 / 1084.0;
const SETTINGS_COLUMN_GAP_PX: f32 = 48.0;
const SETTINGS_SECTION_GAP_PX: f32 = 34.0;
const SETTINGS_TOP_GAP_PX: f32 = 4.0;

pub(super) fn draw_settings(ui: &mut Ui, launcher: &mut Launcher, forms: &mut NetForms) {
    let whole = ui.max_rect();
    let left_width = (whole.width() - SETTINGS_COLUMN_GAP_PX) * SETTINGS_LEFT_SHARE;
    let top = whole.top() + SETTINGS_TOP_GAP_PX;
    let left = Rect::from_min_max(
        pos2(whole.left(), top),
        pos2(whole.left() + left_width, whole.bottom()),
    );
    let right = Rect::from_min_max(
        pos2(left.right() + SETTINGS_COLUMN_GAP_PX, top),
        whole.right_bottom(),
    );
    ui.scope_builder(UiBuilder::new().max_rect(left), |ui| {
        ScrollArea::vertical()
            .id_salt("settings-left")
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                page_section(ui, 0, |ui| draw_game_section(ui, launcher));
                ui.add_space(SETTINGS_SECTION_GAP_PX);
                page_section(ui, 2, |ui| draw_in_game_section(ui, launcher, forms));
            });
    });
    ui.scope_builder(UiBuilder::new().max_rect(right), |ui| {
        ScrollArea::vertical()
            .id_salt("settings-right")
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                page_section(ui, 1, |ui| draw_updates_section(ui, launcher));
                ui.add_space(SETTINGS_SECTION_GAP_PX);
                page_section(ui, 3, |ui| draw_window_section(ui, launcher));
                if launcher.profile.support.saves {
                    ui.add_space(SETTINGS_SECTION_GAP_PX);
                    page_section(ui, 4, |ui| draw_saves_section(ui, launcher));
                }
            });
    });
}
