use hudhook::imgui::Ui;
use nc_hook::menu_view::PANEL_COLUMN_LEFT_PX;
use nc_hook::menu_view::parts::panel_window;

use crate::build_gate::{GateError, VerifiedBuild};
use crate::overlay::mouse_panel::{MouseDiagnostics, draw_mouse_diagnostics};

pub(super) fn draw_diagnostics_window(
    ui: &Ui,
    gate: &Result<VerifiedBuild, GateError>,
    frame: u64,
    mouse: &MouseDiagnostics,
    open: &mut bool,
) {
    if !*open {
        return;
    }
    panel_window(
        ui,
        "diagnostics",
        open,
        [PANEL_COLUMN_LEFT_PX, 16.0],
        [440.0, 400.0],
        || {
            match gate {
                Ok(build) => {
                    ui.text("build gate: pass");
                    ui.text(format!("module base: {:#x}", build.module_base()));
                }
                Err(error) => {
                    ui.text(format!("build gate: FAIL ({error})"));
                    ui.text("module base: unknown");
                }
            }
            ui.text(format!("frame: {frame}"));
            draw_mouse_diagnostics(ui, mouse);
        },
    );
}
