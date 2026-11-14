use eframe::egui::{Label, RichText, Ui};
use replicant_proto::{GUEST_BODY, SaveSummary};

use crate::app::Launcher;
use crate::app::palette::{INK, INK_MUTED};
use crate::app::parts::{ValueLettering, section_heading, value_line};
use crate::app::style::small_font;
use crate::lobby::body_label;
use crate::saves::play_time_short;

const CAMPAIGN_HEADING: &str = "Campaign";
const CAMPAIGN_NOTE: &str = "Kept by the host";
const HEADING_TO_LINES_GAP_PX: f32 = 10.0;
const LINES_TO_NOTE_GAP_PX: f32 = 8.0;
const WAITING_FOR_CARD: &str = "Waiting for the host to pick a save";
const HOST_SAVE_LABEL: &str = "Host's save";
const OWN_SAVE_LABEL: &str = "Your save";
const OWN_BODY_LABEL: &str = "Your character";
const NO_SEAT_YET: &str = "None yet";
const SEAT_KEPT_NOTE: &str =
    "The host keeps this save. It is sent to you at the start and back after every save.";
const NO_SEAT_NOTE: &str = "You continue your own latest co-op save this time.";

pub(super) fn draw_campaign_card(ui: &mut Ui, launcher: &Launcher) {
    section_heading(ui, CAMPAIGN_HEADING, Some(CAMPAIGN_NOTE));
    ui.add_space(HEADING_TO_LINES_GAP_PX);
    let ink = (INK, INK);
    let Some(card) = launcher.campaign_card() else {
        value_line(
            ui,
            WAITING_FOR_CARD,
            "",
            ValueLettering::Body,
            (INK_MUTED, INK),
        );
        return;
    };
    value_line(
        ui,
        HOST_SAVE_LABEL,
        &save_text(&card.host),
        ValueLettering::Body,
        ink,
    );
    let note = match &card.second_seat {
        Some(seat) => {
            value_line(
                ui,
                OWN_SAVE_LABEL,
                &save_text(&seat.summary),
                ValueLettering::Body,
                ink,
            );
            value_line(
                ui,
                OWN_BODY_LABEL,
                body_label(GUEST_BODY),
                ValueLettering::Body,
                ink,
            );
            SEAT_KEPT_NOTE
        }
        None => {
            value_line(
                ui,
                OWN_SAVE_LABEL,
                NO_SEAT_YET,
                ValueLettering::Body,
                (INK, INK_MUTED),
            );
            NO_SEAT_NOTE
        }
    };
    ui.add_space(LINES_TO_NOTE_GAP_PX);
    ui.add(Label::new(RichText::new(note).font(small_font()).color(INK_MUTED)).wrap());
}

fn save_text(summary: &SaveSummary) -> String {
    format!(
        "{}  Lv {}  {}",
        summary.name.trim(),
        summary.level,
        play_time_short(summary.play_seconds)
    )
}
