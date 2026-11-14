use hudhook::imgui::Ui;
use nc_hook::menu_view::PANEL_COLUMN_LEFT_PX;
use nc_hook::menu_view::parts::{choice_line, line_buttons, panel_window, switch_line};
use replicant_game::downed::{
    DownedState, LocalDownedLine, LocalDownedLineInputs, local_downed_line,
};

use crate::downed::{downed_status, request_go_down, request_revive_partner};

use crate::net;
use crate::overlay::enemy_sync_section::draw_enemy_sync;
use crate::overlay::minimap_dot::draw_minimap_tuning;
use crate::overlay::partner_marker::{
    marker_enabled, matrix_method, set_marker_enabled, set_matrix_method,
};
use crate::overlay::party_frame_tuning_panel::draw_party_frame_tuning;
use crate::overlay::spell_size_panel::draw_spell_sizes;
use crate::overlay::stay_together_panel::draw_stay_together;
use crate::overlay::story_relay_section::draw_story_relay;
use crate::puppet::{ghost_enabled, ghost_status, set_ghost_enabled};
use crate::render_camera::{
    OFFSET_AUTO, camera_offset, camera_stats, set_camera_offset, set_tag_sync_uses_drawn_frame,
    stats_text, tag_sync_uses_drawn_frame,
};

#[derive(Debug, Default)]
pub(super) struct CoOpPanel;

impl CoOpPanel {
    pub(super) fn draw(&mut self, ui: &Ui, open: &mut bool) {
        if !*open {
            return;
        }
        panel_window(
            ui,
            "co-op",
            open,
            [PANEL_COLUMN_LEFT_PX, 472.0],
            [440.0, 420.0],
            || {
                draw_party_frame_tuning(ui);
                draw_minimap_tuning(ui);
                draw_spell_sizes(ui);
                ui.separator();
                let Some(net) = net::handle() else {
                    ui.text("launcher link: off (no nier-coop.toml with a link port)");
                    return;
                };
                let view = net.view();
                draw_ghost_controls(ui);
                draw_marker_controls(ui);
                ui.separator();
                draw_downed_controls(ui);
                ui.separator();
                draw_stay_together(ui, &view);
                ui.separator();
                draw_enemy_sync(ui);
                draw_story_relay(ui);
            },
        );
    }
}

fn draw_downed_controls(ui: &Ui) {
    let status = downed_status();
    if !status.session_active {
        ui.text("downed and revive: off until a peer is connected");
        return;
    }
    ui.text(format!(
        "you: {}   partner: {}",
        state_label(status.local),
        state_label(status.partner)
    ));
    match local_downed_line(LocalDownedLineInputs {
        local: status.local,
        game_over: status.game_over,
        bleed_out_progress: status.bleed_out_progress,
        being_revived_progress: status.incoming_revive_progress,
    }) {
        LocalDownedLine::Nothing => {}
        LocalDownedLine::BleedingOut(progress) => ui.text(format!(
            "bleeding out: {:.0}% (partner must revive you)",
            progress * 100.0
        )),
        LocalDownedLine::WaitingForRevive => ui.text("Waiting for revive"),
        LocalDownedLine::BeingRevived(progress) => {
            ui.text(format!("being revived: {:.0}%", progress * 100.0));
        }
    }
    if status.game_over {
        ui.text("both downed: the game over flow is running");
    } else if status.partner == DownedState::Downed {
        ui.text(format!(
            "reviving partner: {:.0}% (stand close)",
            status.revive_progress * 100.0
        ));
    }
    ui.text(format!("Rewinds: {}", status.rewinds));
    match line_buttons(ui, &["revive partner (test)", "go down (test)"], &[]) {
        Some(0) => request_revive_partner(),
        Some(_) => request_go_down(),
        None => {}
    }
}

fn state_label(state: DownedState) -> &'static str {
    match state {
        DownedState::Up => "up",
        DownedState::Downed => "DOWNED",
    }
}

fn draw_ghost_controls(ui: &Ui) {
    let mut enabled = ghost_enabled();
    if switch_line(ui, "show the other player (ghost)", &mut enabled) {
        set_ghost_enabled(enabled);
    }
    let status = ghost_status();
    if status.spawned {
        ui.text("ghost: spawned");
    } else if let Some(reason) = status.wish.why_not() {
        ui.text(format!("ghost: not spawned, {reason}"));
    } else if let Some(error) = &status.spawn_error {
        ui.text(format!("ghost: not spawned, game refused ({error})"));
    } else {
        ui.text("ghost: spawning...");
    }
}

fn draw_marker_controls(ui: &Ui) {
    let mut enabled = marker_enabled();
    if switch_line(ui, "partner name and edge arrow", &mut enabled) {
        set_marker_enabled(enabled);
    }
    draw_camera_offset_controls(ui);
    let mut matrix = matrix_method();
    if switch_line(
        ui,
        "marker: use camera matrix instead of view-projection",
        &mut matrix,
    ) {
        set_matrix_method(matrix);
    }
}

fn draw_camera_offset_controls(ui: &Ui) {
    let current = camera_offset();
    ui.text("camera frame offset (find where the tag sticks while turning):");
    let offsets = [OFFSET_AUTO, 0, 1, 2, 3];
    let shown = offsets.iter().position(|offset| *offset == current);
    if let Some(index) = choice_line(ui, "", &["auto", "0", "1", "2", "3"], shown)
        && let Some(offset) = offsets.get(index)
    {
        set_camera_offset(*offset);
    }
    let uses_drawn_frame = tag_sync_uses_drawn_frame();
    let tag_sync = usize::from(uses_drawn_frame);
    if let Some(index) = choice_line(ui, "Name tag sync:", &["old", "new"], Some(tag_sync)) {
        set_tag_sync_uses_drawn_frame(index == 1);
    }
    ui.text_wrapped(stats_text(&camera_stats()));
}
