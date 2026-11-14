use eframe::egui::Ui;
use nc_launcher_core::{
    LobbyOption, MAX_ENEMY_DAMAGE_PERCENT, MAX_ENEMY_HEALTH_PERCENT, SessionOptions,
    lobby_permissions, option_available,
};
use nc_ui_style::{ENEMY_DAMAGE_METER_CELLS, ENEMY_HEALTH_METER_CELLS};

use crate::app::parts::{ControlLook, OptionNesting, meter_line, option_line, section_heading};
use crate::app::{Launcher, NetForms};

const OPTIONS_HEADING: &str = "Options";
const HEADING_TO_OPTIONS_GAP_PX: f32 = 8.0;

pub(super) fn draw_lobby_options(ui: &mut Ui, launcher: &mut Launcher, forms: &mut NetForms) {
    let editable = lobby_permissions(launcher.link.guest).change_options;
    let mut options = if editable {
        launcher.config.session_options()
    } else {
        launcher.host_options_seen.unwrap_or_default()
    };
    section_heading(ui, OPTIONS_HEADING, None);
    ui.add_space(HEADING_TO_OPTIONS_GAP_PX);
    let changed = draw_option_lines(ui, &mut options, editable);
    if changed && editable {
        launcher.config.take_session_options(options);
        launcher.apply_settings();
        forms.session_options = launcher.config.session_options();
        forms.push_session_options();
    }
}

fn draw_option_lines(ui: &mut Ui, options: &mut SessionOptions, editable: bool) -> bool {
    let shown = *options;
    let look = |option| {
        if !option_available(&shown, option) {
            ControlLook::Unavailable
        } else if editable {
            ControlLook::Editable
        } else {
            ControlLook::ReadOnly
        }
    };
    let mut changed = false;
    changed |= option_line(
        ui,
        "Names over players",
        &mut options.show_names,
        look(LobbyOption::NamesOverPlayers),
        OptionNesting::Top,
    )
    .changed();
    changed |= option_line(
        ui,
        "Distance",
        &mut options.show_distance,
        look(LobbyOption::Distance),
        OptionNesting::Branch,
    )
    .changed();
    changed |= option_line(
        ui,
        "Ping",
        &mut options.show_ping,
        look(LobbyOption::Ping),
        OptionNesting::LastBranch,
    )
    .changed();
    changed |= option_line(
        ui,
        "Party frame",
        &mut options.show_party_frame,
        look(LobbyOption::PartyFrame),
        OptionNesting::Top,
    )
    .changed();
    changed |= meter_line(
        ui,
        "Enemy health per extra player",
        &mut options.enemy_health_percent,
        MAX_ENEMY_HEALTH_PERCENT,
        ENEMY_HEALTH_METER_CELLS,
        look(LobbyOption::EnemyHealth),
    );
    changed |= meter_line(
        ui,
        "Enemy damage per extra player",
        &mut options.enemy_damage_percent,
        MAX_ENEMY_DAMAGE_PERCENT,
        ENEMY_DAMAGE_METER_CELLS,
        look(LobbyOption::EnemyDamage),
    );
    changed
}
