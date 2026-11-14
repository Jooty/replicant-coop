use hudhook::imgui::Ui;
use nc_hook::menu_view::parts::{MeterStep, meter_line, sub_heading, switch_line};
use nc_ui_style::ControlLook;
use replicant_game::stay_together::{MAX_LEASH_UNITS, MIN_LEASH_UNITS};
use replicant_game::zone_rules::{MAX_ZONE_RADIUS_UNITS, MIN_ZONE_RADIUS_UNITS};

use crate::net::NetShared;
use crate::stay_together::{follow_enabled, leash, set_follow_enabled, set_leash, stay_status};
use crate::zone_rules::{set_zone_radius, zone_radius};

const UNITS_METER_CELLS: u32 = 12;

pub(super) fn draw_stay_together(ui: &Ui, view: &NetShared) {
    sub_heading(ui, "stay together");
    let mut follow = follow_enabled();
    if switch_line(ui, "follow the host's map changes", &mut follow) {
        set_follow_enabled(follow);
    }
    draw_role_and_status(ui, view);
    if let Some(units) = units_meter(
        ui,
        "everyone nearby radius (units)",
        zone_radius().units(),
        (MIN_ZONE_RADIUS_UNITS, MAX_ZONE_RADIUS_UNITS),
    ) {
        set_zone_radius(units);
    }
    if let Some(units) = units_meter(
        ui,
        "leash (units)",
        leash().units(),
        (MIN_LEASH_UNITS, MAX_LEASH_UNITS),
    ) {
        set_leash(units);
    }
}

fn units_meter(ui: &Ui, label: &str, units: f32, range: (f32, f32)) -> Option<f32> {
    let mut shown = units.round() as i32;
    let range = (range.0.round() as i32, range.1.round() as i32);
    meter_line(
        ui,
        label,
        &mut shown,
        range,
        UNITS_METER_CELLS,
        MeterStep::Exact,
        "",
        ControlLook::Editable,
    )
    .then_some(shown as f32)
}

fn draw_role_and_status(ui: &Ui, view: &NetShared) {
    ui.text(match view.local_slot {
        Some(0) => "you are the host: you change zones only when everyone is nearby",
        Some(_) => "you are the guest: only the host can change zones",
        None => "role: unknown until connected",
    });
    let status = stay_status();
    if !status.follow.is_empty() {
        ui.text_wrapped(&status.follow);
    }
    if status.blocked_exits > 0 {
        ui.text(format!("zone changes blocked: {}", status.blocked_exits));
    }
}
