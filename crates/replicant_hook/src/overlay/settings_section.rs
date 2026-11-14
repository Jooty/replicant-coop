use hudhook::imgui::Ui;
use nc_hook::menu_view::MENU_GROUP_GAP_PX;
use nc_hook::menu_view::parts::{MeterStep, meter_line, note_line, option_line};
use nc_ui_style::{
    ControlLook, ENEMY_DAMAGE_METER_CELLS, ENEMY_HEALTH_METER_CELLS, INK_MUTED, OptionNesting,
};
use replicant_proto::{MAX_ENEMY_DAMAGE_PERCENT, MAX_ENEMY_HEALTH_PERCENT, SessionOptions};

use crate::net::{self, NetState};
use crate::paused_hits::{paused_players_can_be_hit, set_paused_players_can_be_hit};
use crate::player_settings::{apply, current, publish_host_options, publish_to_launcher};

const PERCENT_UNIT: &str = " %";

pub(super) fn draw_settings_section(ui: &Ui) {
    let before = current();
    let mut settings = before;
    let editable = ControlLook::Editable;
    let top = OptionNesting::Top;
    option_line(
        ui,
        "Keep running in the background",
        &mut settings.keep_running,
        editable,
        top,
    );
    option_line(
        ui,
        "Show the event log",
        &mut settings.event_log,
        editable,
        top,
    );
    if settings != before {
        apply(settings);
        publish_to_launcher();
    }
    ui.dummy([0.0, MENU_GROUP_GAP_PX]);
    let view = net::handle().map(|net| net.view());
    draw_host_only_options(
        ui,
        view.as_ref().and_then(|view| view.local_slot),
        view.as_ref().map(|view| view.options),
    );
    let linked = view
        .as_ref()
        .is_some_and(|view| view.state != NetState::WaitingForLauncher);
    note_line(
        ui,
        if linked {
            "Shared with the launcher: change them here or there."
        } else {
            "The launcher is not linked: changes last until the game closes."
        },
        INK_MUTED,
    );
}

fn draw_host_only_options(ui: &Ui, slot: Option<u8>, options: Option<SessionOptions>) {
    let options = options.unwrap_or_default();
    let is_host = slot == Some(0);
    let look = if is_host {
        ControlLook::Editable
    } else {
        ControlLook::ReadOnly
    };
    let mut edited = options;
    option_line(
        ui,
        "Show names over players",
        &mut edited.show_names,
        look,
        OptionNesting::Top,
    );
    option_line(
        ui,
        "Show distance",
        &mut edited.show_distance,
        look,
        OptionNesting::Branch,
    );
    option_line(
        ui,
        "Show ping",
        &mut edited.show_ping,
        look,
        OptionNesting::LastBranch,
    );
    option_line(
        ui,
        "Show party frame",
        &mut edited.show_party_frame,
        look,
        OptionNesting::Top,
    );
    edited.enemy_health_percent = percent_meter(
        ui,
        "Enemy health per extra player",
        edited.enemy_health_percent,
        MAX_ENEMY_HEALTH_PERCENT,
        ENEMY_HEALTH_METER_CELLS,
        look,
    );
    edited.enemy_damage_percent = percent_meter(
        ui,
        "Enemy damage per extra player",
        edited.enemy_damage_percent,
        MAX_ENEMY_DAMAGE_PERCENT,
        ENEMY_DAMAGE_METER_CELLS,
        look,
    );
    let mut hittable = paused_players_can_be_hit();
    if option_line(
        ui,
        "Paused players can be hit",
        &mut hittable,
        look,
        OptionNesting::Top,
    ) {
        set_paused_players_can_be_hit(hittable);
    }
    if is_host {
        if edited != options {
            publish_host_options(edited);
        }
        return;
    }
    note_line(
        ui,
        if slot.is_some() {
            "These are the host's choice."
        } else {
            "The host chooses these once someone joins."
        },
        INK_MUTED,
    );
}

fn percent_meter(ui: &Ui, label: &str, value: u16, max: u16, cells: u32, look: ControlLook) -> u16 {
    let mut shown = i32::from(value);
    meter_line(
        ui,
        label,
        &mut shown,
        (0, i32::from(max)),
        cells,
        MeterStep::Cells,
        PERCENT_UNIT,
        look,
    );
    u16::try_from(shown).unwrap_or(value).min(max)
}
