use eframe::egui::{self, Rect, Ui, UiBuilder, vec2};

use crate::app::motion::page_section;
use crate::app::pages::game_row::draw_game_row;
use crate::app::pages::lobby::session_open;
use crate::app::parts::{DOUBLE_RULE_HEIGHT_PX, double_rule};
use crate::app::{Launcher, NetForms, Tab};

const ROW_TO_RULE_GAP_PX: f32 = 8.0;
const HOME_BOTTOM_EXTRA_GAP_PX: f32 = 12.0;
const MIN_ROW_HEIGHT_PX: f32 = 300.0;
const MIN_ROWS_SHARING_THE_PAGE: f32 = 2.0;

pub(super) fn draw_home(ui: &mut Ui, tab: &mut Tab, launcher: &mut Launcher, forms: &mut NetForms) {
    let switch_locked = !launcher.can_switch_game() || session_open(forms);
    let cards = launcher.cards.clone();
    // A single row takes half of the page, not all of it.
    let count = (cards.len() as f32).max(MIN_ROWS_SHARING_THE_PAGE);
    let between = 2.0 * ROW_TO_RULE_GAP_PX + DOUBLE_RULE_HEIGHT_PX;
    let room = ui.available_height() - HOME_BOTTOM_EXTRA_GAP_PX - between * (count - 1.0);
    // Every row is the same height whatever it holds.
    let row_height = (room / count).max(MIN_ROW_HEIGHT_PX);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (index, card) in cards.iter().enumerate() {
                if index > 0 {
                    ui.add_space(ROW_TO_RULE_GAP_PX);
                    double_rule(ui);
                    ui.add_space(ROW_TO_RULE_GAP_PX);
                }
                let rect =
                    Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), row_height));
                ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                    ui.set_min_size(rect.size());
                    page_section(ui, index, |ui| {
                        draw_game_row(ui, rect, tab, launcher, forms, card, switch_locked);
                    });
                });
            }
        });
}
