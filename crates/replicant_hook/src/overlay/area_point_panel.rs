use hudhook::imgui::Ui;
use nc_hook::menu_view::PANEL_COLUMN_LEFT_PX;
use nc_hook::menu_view::parts::{note_line, panel_window};
use nc_ui_style::INK_MUTED;
use replicant_game::offsets::MAX_PHASE_POINT;

pub(super) fn draw_area_point_panel(ui: &Ui, point: &mut i32, open: &mut bool) {
    if !*open {
        return;
    }
    panel_window(
        ui,
        "area point",
        open,
        [PANEL_COLUMN_LEFT_PX, 16.0],
        [320.0, 130.0],
        || {
            ui.input_int("point", point).build();
            *point = (*point).clamp(0, MAX_PHASE_POINT);
            note_line(
                ui,
                "the spawn point the map teleporter asks the game for",
                INK_MUTED,
            );
        },
    );
}
