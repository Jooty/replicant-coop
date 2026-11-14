use eframe::egui::{Align2, FontId, Id, Rect, Response, Sense, Stroke, Ui, UiBuilder, pos2, vec2};

use crate::app::motion::sweep_toward;
use crate::app::pages::game_title::game_title_job;
use crate::app::palette::{HAIRLINE_STRONG, HOVER_WASH, INK, INK_MUTED, INK_ON_SAND};
use crate::app::parts::{
    caps_galley, paint_in_two_inks, paint_sweep_fill, paint_triangle_left, spaced_job, text_control,
};
use crate::app::style::light_family;
use crate::app::{Launcher, Screen, Tab, WINDOW_TITLE, game_look};

pub(super) const TOP_BAR_HEIGHT_PX: f32 = 60.0;
const BACK_TRIANGLE_WIDTH_PX: f32 = 7.0;
const BACK_TRIANGLE_TO_TEXT_PX: f32 = 9.0;
const BACK_RIGHT_PADDING_PX: f32 = 22.0;
const BACK_DIVIDER_HEIGHT_PX: f32 = 28.0;
const TITLE_LEFT_GAP_PX: f32 = 22.0;
const TITLE_COLUMN_WIDTH_PX: f32 = 300.0;
const TITLE_TO_TABS_MIN_GAP_PX: f32 = 24.0;
const TOP_BAR_GARAMOND_TITLE_PX: f32 = 28.0;
const TOP_BAR_CAPS_TITLE_PX: f32 = 22.0;
const TAB_HEIGHT_PX: f32 = 32.0;
const TAB_GAP_PX: f32 = 6.0;
const TAB_SIDE_PADDING_PX: f32 = 18.0;
const TAB_SHADOW_OFFSET_PX: f32 = 3.0;
const HOME_TITLE_SIZE_PX: f32 = 22.0;
const HOME_TITLE_LETTER_SPACING_EM: f32 = 0.26;
const LAUNCHER_VERSION_PREFIX: &str = "Launcher ";
const TAB_SWEEP_SALT: &str = "tab-sweep";
const LOGS_LABEL: &str = "Logs";
const LOGS_DIVIDER_GAP_PX: f32 = 22.0;
const LOGS_DIVIDER_HEIGHT_PX: f32 = 28.0;

pub(super) fn draw_home_bar(ui: &mut Ui, launcher: &mut Launcher) {
    let rect = ui.max_rect();
    let middle = rect.center().y;
    let logs = caps_galley(ui.painter(), LOGS_LABEL, INK);
    let logs_rect = Align2::RIGHT_CENTER.anchor_size(pos2(rect.right(), middle), logs.size());
    let logs_clicked = ui
        .scope_builder(UiBuilder::new().max_rect(logs_rect), |ui| {
            text_control(ui, LOGS_LABEL, true).clicked()
        })
        .inner;
    if logs_clicked {
        launcher.screen = Screen::Logs;
    }
    let divider_x = logs_rect.left() - LOGS_DIVIDER_GAP_PX;
    let painter = ui.painter();
    painter.vline(
        divider_x + 0.5,
        middle - LOGS_DIVIDER_HEIGHT_PX / 2.0..=middle + LOGS_DIVIDER_HEIGHT_PX / 2.0,
        Stroke::new(1.0, HAIRLINE_STRONG),
    );
    let title = painter.layout_job(spaced_job(
        &WINDOW_TITLE.to_uppercase(),
        FontId::new(HOME_TITLE_SIZE_PX, light_family()),
        INK,
        HOME_TITLE_LETTER_SPACING_EM,
    ));
    painter.galley(pos2(rect.left(), middle - title.size().y / 2.0), title, INK);
    let version = caps_galley(
        painter,
        &format!("{LAUNCHER_VERSION_PREFIX}{}", env!("CARGO_PKG_VERSION")),
        INK_MUTED,
    );
    let at = Align2::RIGHT_CENTER
        .anchor_size(
            pos2(divider_x - LOGS_DIVIDER_GAP_PX, middle),
            version.size(),
        )
        .min;
    painter.galley(at, version, INK_MUTED);
}

pub(super) fn draw_logs_bar(ui: &mut Ui, launcher: &mut Launcher) {
    let rect = ui.max_rect();
    let middle = rect.center().y;
    let back = back_button(ui, rect.left(), middle);
    if back.clicked() {
        launcher.go_home();
    }
    let title = ui.painter().layout_job(spaced_job(
        &LOGS_LABEL.to_uppercase(),
        FontId::new(HOME_TITLE_SIZE_PX, light_family()),
        INK,
        HOME_TITLE_LETTER_SPACING_EM,
    ));
    ui.painter().galley(
        pos2(
            back.rect.right() + TITLE_LEFT_GAP_PX,
            middle - title.size().y / 2.0,
        ),
        title,
        INK,
    );
}

pub(super) fn draw_top_bar(
    ui: &mut Ui,
    tab: &mut Tab,
    launcher: &mut Launcher,
    shown: &[(Tab, &str)],
) {
    let rect = ui.max_rect();
    let middle = rect.center().y;
    let back = back_button(ui, rect.left(), middle);
    if back.clicked() {
        launcher.go_home();
    }
    let tabs_width = tabs_width(ui, shown);
    let title_left = back.rect.right() + TITLE_LEFT_GAP_PX;
    let face = game_look(launcher.profile).title_face;
    let title = ui.painter().layout_job(game_title_job(
        launcher.profile.display_name,
        face,
        TOP_BAR_GARAMOND_TITLE_PX,
        TOP_BAR_CAPS_TITLE_PX,
        INK,
    ));
    let room_for_title = rect.right() - tabs_width - title_left;
    let title_column = TITLE_COLUMN_WIDTH_PX
        .min(room_for_title)
        .max(title.size().x + TITLE_TO_TABS_MIN_GAP_PX);
    let title_rect = Rect::from_min_max(
        pos2(title_left, rect.top()),
        pos2(
            title_left + title_column - TITLE_TO_TABS_MIN_GAP_PX / 2.0,
            rect.bottom(),
        ),
    );
    let title_top = middle - title.size().y / 2.0;
    ui.painter()
        .with_clip_rect(title_rect)
        .galley(pos2(title_left, title_top), title, INK);
    let tabs_rect = Rect::from_min_size(
        pos2(title_left + title_column, middle - TAB_HEIGHT_PX / 2.0),
        vec2(tabs_width, TAB_HEIGHT_PX),
    );
    ui.scope_builder(UiBuilder::new().max_rect(tabs_rect), |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = TAB_GAP_PX;
            for &(value, label) in shown {
                if tab_button(ui, label, *tab == value).clicked() {
                    *tab = value;
                }
            }
        });
    });
}

fn back_button(ui: &mut Ui, left: f32, middle: f32) -> Response {
    let galley = caps_galley(ui.painter(), "Home", INK);
    let width =
        BACK_TRIANGLE_WIDTH_PX + BACK_TRIANGLE_TO_TEXT_PX + galley.size().x + BACK_RIGHT_PADDING_PX;
    let rect = Rect::from_min_size(
        pos2(left, middle - BACK_DIVIDER_HEIGHT_PX / 2.0),
        vec2(width, BACK_DIVIDER_HEIGHT_PX),
    );
    let response = ui.interact(rect, Id::unique("top-bar-back-home"), Sense::click());
    let color = INK;
    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(rect, 0.0, HOVER_WASH);
    }
    paint_triangle_left(painter, pos2(left, middle), color);
    let text_left = left + BACK_TRIANGLE_WIDTH_PX + BACK_TRIANGLE_TO_TEXT_PX;
    painter.galley(
        pos2(text_left, middle - galley.size().y / 2.0),
        galley,
        color,
    );
    painter.vline(
        rect.right() - 0.5,
        rect.y_range(),
        Stroke::new(1.0, HAIRLINE_STRONG),
    );
    response
}

fn tabs_width(ui: &Ui, shown: &[(Tab, &str)]) -> f32 {
    let gaps = TAB_GAP_PX * shown.len().saturating_sub(1) as f32;
    shown
        .iter()
        .map(|(_, label)| {
            caps_galley(ui.painter(), label, INK).size().x + 2.0 * TAB_SIDE_PADDING_PX
        })
        .sum::<f32>()
        + gaps
}

fn tab_button(ui: &mut Ui, label: &str, selected: bool) -> Response {
    let galley = caps_galley(ui.painter(), label, INK);
    let size = vec2(galley.size().x + 2.0 * TAB_SIDE_PADDING_PX, TAB_HEIGHT_PX);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let sweep = sweep_toward(ui, response.id.with(TAB_SWEEP_SALT), selected);
    let painter = ui.painter();
    let text_at = rect.center() - galley.size() / 2.0;
    let covered = paint_sweep_fill(painter, rect, sweep, TAB_SHADOW_OFFSET_PX);
    let rest_ink = if response.hovered() || selected {
        INK
    } else {
        INK_MUTED
    };
    paint_in_two_inks(painter, rect, covered, |painter, on_sand| {
        let ink = if on_sand { INK_ON_SAND } else { rest_ink };
        painter.galley_with_override_text_color(text_at, galley.clone(), ink);
    });
    response
}
