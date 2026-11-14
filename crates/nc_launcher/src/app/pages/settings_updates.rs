use eframe::egui::{Align, Layout, Rect, Sense, Ui, UiBuilder, pos2, vec2};

use crate::app::palette::{INK, INK_MUTED, NOT_READY, READY};
use crate::app::parts::{
    ControlLook, LINE_BUTTON_HEIGHT_PX, LineTone, OptionNesting, UpdateLineState, ValueLettering,
    field_line, line_button, option_line, section_heading, update_line, value_line,
};
use crate::app::style::body_font;
use crate::app::{Launcher, UpdateState};
use crate::update::{BUILD_COMMIT, own_channel_label, short_commit};

const HEADING_TO_OPTIONS_PX: f32 = 8.0;
const BUTTONS_TOP_GAP_PX: f32 = 16.0;
const BUTTON_GAP_PX: f32 = 14.0;
const BUTTONS_LEFT_PX: f32 = 12.0;
const SERVER_LINE_HEIGHT_PX: f32 = 30.0;
const SERVER_LABEL_LEFT_PX: f32 = 12.0;
const SERVER_FIELD_WIDTH_PX: f32 = 200.0;
const SERVER_HINT: &str = "host:port";
const STATE_TOP_GAP_PX: f32 = 10.0;
const UPDATE_LINE_INSET_PX: f32 = 12.0;
const UPDATE_LINE_WIDTH_PX: f32 = 384.0;

pub(super) fn draw_updates_section(ui: &mut Ui, launcher: &mut Launcher) {
    section_heading(ui, "Updates", None);
    ui.add_space(HEADING_TO_OPTIONS_PX);
    let version = format!(
        "{} ({})",
        env!("CARGO_PKG_VERSION"),
        short_commit(BUILD_COMMIT)
    );
    value_line(ui, "Launcher", &version, ValueLettering::Body, (INK, INK));
    value_line(
        ui,
        "Channel",
        &own_channel_label(),
        ValueLettering::Capitals,
        (INK, INK),
    );
    let has_server = launcher.update_url().is_some();
    let server_look = if has_server {
        ControlLook::Editable
    } else {
        ControlLook::Unavailable
    };
    if option_line(
        ui,
        "Check for updates at launch",
        &mut launcher.config.check_for_updates,
        server_look,
        OptionNesting::Top,
    )
    .changed()
    {
        launcher.apply_settings();
    }
    draw_server_line(ui, launcher);
    let logs_look = if has_server && launcher.log_upload_available() {
        ControlLook::Editable
    } else {
        ControlLook::Unavailable
    };
    if option_line(
        ui,
        "Send logs after each game",
        &mut launcher.config.send_logs,
        logs_look,
        OptionNesting::LastBranch,
    )
    .changed()
    {
        launcher.apply_settings();
    }
    ui.add_space(BUTTONS_TOP_GAP_PX);
    draw_update_buttons(ui, launcher);
    draw_update_state(ui, launcher);
}

fn draw_server_line(ui: &mut Ui, launcher: &mut Launcher) {
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), SERVER_LINE_HEIGHT_PX),
        Sense::hover(),
    );
    ui.painter().text(
        pos2(rect.left() + SERVER_LABEL_LEFT_PX, rect.center().y),
        eframe::egui::Align2::LEFT_CENTER,
        "Build server",
        body_font(),
        INK,
    );
    let default_text = launcher.update_url().unwrap_or_default();
    let mut server_text = launcher.updater.server_text.take().unwrap_or(default_text);
    let field = Rect::from_min_max(
        pos2(rect.right() - SERVER_FIELD_WIDTH_PX, rect.top()),
        rect.max,
    );
    let changed = ui
        .scope_builder(
            UiBuilder::new()
                .max_rect(field)
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                field_line(
                    ui,
                    &mut server_text,
                    SERVER_FIELD_WIDTH_PX,
                    SERVER_HINT,
                    body_font(),
                )
                .changed()
            },
        )
        .inner;
    if changed {
        let trimmed = server_text.trim().to_owned();
        launcher.config.update_url = (!trimmed.is_empty()).then_some(trimmed);
        launcher.apply_settings();
    }
    launcher.updater.server_text = Some(server_text);
}

fn draw_update_buttons(ui: &mut Ui, launcher: &mut Launcher) {
    let ctx = ui.ctx().clone();
    let checking = matches!(
        launcher.updater.state,
        UpdateState::Checking | UpdateState::Updating
    );
    let has_server = launcher.update_url().is_some();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = BUTTON_GAP_PX;
        ui.add_space(BUTTONS_LEFT_PX);
        if line_button(
            ui,
            "Check for updates",
            LineTone::Plain,
            LINE_BUTTON_HEIGHT_PX,
            None,
            has_server && !checking,
        )
        .clicked()
        {
            launcher.start_update_check(&ctx);
        }
    });
}

fn draw_update_state(ui: &mut Ui, launcher: &mut Launcher) {
    let control = match &launcher.updater.state {
        UpdateState::Available(update) => Some((
            format!("Update ready: {}", short_commit(&update.manifest.commit)),
            UpdateLineState::Offered,
        )),
        UpdateState::Updating => Some((
            "Downloading and installing".to_owned(),
            UpdateLineState::Running,
        )),
        _ => None,
    };
    let plain = match &launcher.updater.state {
        UpdateState::Checking => Some(("Checking".to_owned(), INK_MUTED)),
        UpdateState::UpToDate => Some(("Up to date".to_owned(), READY)),
        UpdateState::Updated(short) => Some((format!("Mod updated to {short}"), READY)),
        UpdateState::Failed(message) => Some((message.clone(), NOT_READY)),
        _ => None,
    };
    ui.add_space(STATE_TOP_GAP_PX);
    if let Some((text, state)) = control {
        let clicked = ui
            .horizontal(|ui| {
                ui.add_space(BUTTONS_LEFT_PX - UPDATE_LINE_INSET_PX);
                update_line(
                    ui,
                    "settings-updates-update",
                    &text,
                    UPDATE_LINE_WIDTH_PX,
                    state,
                )
                .clicked()
            })
            .inner;
        if clicked {
            let ctx = ui.ctx().clone();
            launcher.start_update_apply(&ctx, false);
        }
        return;
    }
    let Some((text, ink)) = plain else {
        return;
    };
    ui.horizontal(|ui| {
        ui.add_space(BUTTONS_LEFT_PX);
        ui.label(
            eframe::egui::RichText::new(text)
                .font(body_font())
                .color(ink),
        );
    });
}
