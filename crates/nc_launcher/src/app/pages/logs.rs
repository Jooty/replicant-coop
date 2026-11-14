use eframe::egui::{Label, RichText, ScrollArea, Sense, Stroke, Ui, vec2};

use crate::app::Launcher;
use crate::app::motion::page_section;
use crate::app::palette::{HAIRLINE, INK, INK_MUTED, INK_UNAVAILABLE};
use crate::app::parts::{LINE_BUTTON_HEIGHT_PX, LineTone, line_button, section_heading};
use crate::app::style::{body_font, small_font};

const PAGE_TOP_GAP_PX: f32 = 4.0;
const BUTTON_GAP_PX: f32 = 14.0;
const SIDE_INSET_PX: f32 = 12.0;
const COMMANDS_TO_LOG_GAP_PX: f32 = 22.0;
const LOG_LINE_PADDING_PX: f32 = 6.0;
const LOG_LINE_MIN_HEIGHT_PX: f32 = 30.0;

pub(super) fn draw_logs(ui: &mut Ui, launcher: &mut Launcher) {
    ui.spacing_mut().item_spacing.y = 0.0;
    ui.add_space(PAGE_TOP_GAP_PX);
    page_section(ui, 0, |ui| draw_commands(ui, launcher));
    ui.add_space(COMMANDS_TO_LOG_GAP_PX);
    page_section(ui, 1, |ui| draw_log(ui, launcher));
}

fn draw_log(ui: &mut Ui, launcher: &Launcher) {
    let lines = launcher.log.len().to_string();
    section_heading(ui, "Launcher log", Some(&format!("{lines} lines")));
    ScrollArea::vertical()
        .id_salt("launcher-log")
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if launcher.log.is_empty() {
                log_line(ui, "Nothing logged yet", INK_UNAVAILABLE);
            }
            for line in &launcher.log {
                log_line(ui, line, INK);
            }
        });
}

fn draw_commands(ui: &mut Ui, launcher: &mut Launcher) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = BUTTON_GAP_PX;
        ui.add_space(SIDE_INSET_PX);
        let button = |ui: &mut Ui, label: &str, enabled: bool| {
            line_button(
                ui,
                label,
                LineTone::Plain,
                LINE_BUTTON_HEIGHT_PX,
                None,
                enabled,
            )
            .clicked()
        };
        if button(ui, "Copy log", true) {
            ui.ctx().copy_text(launcher.log.join("\n"));
        }
        if button(ui, "Export logs", true) {
            launcher.export_logs();
        }
        if launcher.log_upload_available()
            && button(ui, "Send logs now", !launcher.log_upload_busy())
        {
            launcher.send_logs_now(ui.ctx());
        }
        if button(ui, "Open logs folder", true) {
            launcher.open_logs_folder();
        }
        if let Some(note) = &launcher.export_note {
            ui.add(Label::new(RichText::new(note).font(small_font()).color(INK_MUTED)).truncate());
        }
    });
}

fn log_line(ui: &mut Ui, text: &str, ink: eframe::egui::Color32) {
    let width = ui.available_width();
    let galley = ui.painter().layout(
        text.to_owned(),
        body_font(),
        ink,
        width - 2.0 * SIDE_INSET_PX,
    );
    let height = (galley.size().y + 2.0 * LOG_LINE_PADDING_PX).max(LOG_LINE_MIN_HEIGHT_PX);
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let top = rect.center().y - galley.size().y / 2.0;
    ui.painter().galley(
        eframe::egui::pos2(rect.left() + SIDE_INSET_PX, top),
        galley,
        ink,
    );
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, HAIRLINE),
    );
}
