use eframe::egui::{ScrollArea, Ui};

use crate::app::Launcher;
use crate::app::palette::{INK_MUTED, INK_UNAVAILABLE};
use crate::app::parts::{
    LINE_BUTTON_HEIGHT_PX, LineTone, MenuValue, StepDirection, caps_label, line_button, menu_row,
    section_heading, stepper,
};
use crate::app::style::body_font;
use crate::saves::{SlotSummary, slot_label, stamp_label};

const SUB_HEAD_TOP_GAP_PX: f32 = 12.0;
const SUB_HEAD_TO_LIST_PX: f32 = 8.0;
const BACKUPS_TOP_GAP_PX: f32 = 22.0;
const BUTTONS_TOP_GAP_PX: f32 = 10.0;
const BUTTON_GAP_PX: f32 = 14.0;
const SIDE_INSET_PX: f32 = 12.0;
const BACKUP_LIST_MAX_HEIGHT_PX: f32 = 204.0;

pub(super) fn draw_saves_section(ui: &mut Ui, launcher: &mut Launcher) {
    section_heading(ui, "Saves", Some("Copy and back up"));
    sub_head(ui, "From my saves", SUB_HEAD_TOP_GAP_PX);
    draw_copy_from(ui, launcher);
    let idle = !launcher.busy();
    ui.add_space(BUTTONS_TOP_GAP_PX);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = BUTTON_GAP_PX;
        ui.add_space(SIDE_INSET_PX);
        let can_copy = idle && !launcher.any_game_running && launcher.copy_from_slot.is_some();
        if line_button(
            ui,
            "Copy into co-op",
            LineTone::Plain,
            LINE_BUTTON_HEIGHT_PX,
            None,
            can_copy,
        )
        .clicked()
        {
            launcher.copy_normal_slot_into_coop();
        }
        if line_button(
            ui,
            "Back up my saves",
            LineTone::Plain,
            LINE_BUTTON_HEIGHT_PX,
            None,
            launcher.game.is_some() && idle,
        )
        .clicked()
        {
            launcher.backup_saves();
        }
    });
    sub_head(ui, "Co-op backups", BACKUPS_TOP_GAP_PX);
    draw_backup_list(ui, launcher);
    ui.add_space(BUTTONS_TOP_GAP_PX);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = BUTTON_GAP_PX;
        ui.add_space(SIDE_INSET_PX);
        if line_button(
            ui,
            "Back up now",
            LineTone::Plain,
            LINE_BUTTON_HEIGHT_PX,
            None,
            launcher.game.is_some() && idle,
        )
        .clicked()
        {
            launcher.backup_coop_saves();
        }
        let can_restore = idle && !launcher.any_game_running && launcher.selected_backup.is_some();
        if line_button(
            ui,
            "Restore",
            LineTone::Plain,
            LINE_BUTTON_HEIGHT_PX,
            None,
            can_restore,
        )
        .clicked()
        {
            launcher.restore_selected_backup();
        }
    });
}

fn sub_head(ui: &mut Ui, text: &str, gap: f32) {
    ui.add_space(gap);
    ui.horizontal(|ui| {
        ui.add_space(SIDE_INSET_PX);
        caps_label(ui, text, INK_MUTED);
    });
    ui.add_space(SUB_HEAD_TO_LIST_PX);
}

fn draw_copy_from(ui: &mut Ui, launcher: &mut Launcher) {
    let used: Vec<SlotSummary> = launcher
        .normal_slots
        .iter()
        .filter(|slot| slot.used)
        .cloned()
        .collect();
    ui.horizontal(|ui| {
        ui.add_space(SIDE_INSET_PX);
        if used.is_empty() {
            launcher.copy_from_slot = None;
            ui.label(
                eframe::egui::RichText::new("No saves of your own were found")
                    .font(body_font())
                    .color(INK_UNAVAILABLE),
            );
            return;
        }
        let at = used
            .iter()
            .position(|slot| Some(slot.index) == launcher.copy_from_slot)
            .unwrap_or(0);
        let slot = &used[at];
        let value = format!("Slot {}  {}", slot_label(slot.index), slot.name.trim());
        let note = format!("Lv {}, {}", slot.level, slot.play_time_short());
        let picked = match stepper(ui, &value, Some(&note), used.len() > 1) {
            Some(StepDirection::Back) => (at + used.len() - 1) % used.len(),
            Some(StepDirection::Forward) => (at + 1) % used.len(),
            None => at,
        };
        launcher.copy_from_slot = Some(used[picked].index);
    });
}

fn draw_backup_list(ui: &mut Ui, launcher: &mut Launcher) {
    if launcher.backups.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(SIDE_INSET_PX);
            ui.label(
                eframe::egui::RichText::new("No backups yet")
                    .font(body_font())
                    .color(INK_UNAVAILABLE),
            );
        });
        return;
    }
    ScrollArea::vertical()
        .id_salt("settings-backups")
        .max_height(BACKUP_LIST_MAX_HEIGHT_PX)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for stamp in launcher.backups.clone() {
                let selected = launcher.selected_backup.as_deref() == Some(stamp.as_str());
                if menu_row(
                    ui,
                    &stamp_label(&stamp),
                    MenuValue::Plain("Co-op save"),
                    selected,
                    true,
                )
                .clicked()
                {
                    launcher.selected_backup = Some(stamp);
                }
            }
        });
}
