use eframe::egui::Ui;

use crate::app::Launcher;
use crate::app::parts::{
    ControlLook, OptionNesting, StepDirection, option_line, section_heading, stepper_line,
};

const HEADING_TO_OPTIONS_PX: f32 = 8.0;
const MOTION_LABEL: &str = "Animate menus";
const SIZE_LABEL: &str = "Size";

pub(super) fn draw_window_section(ui: &mut Ui, launcher: &mut Launcher) {
    section_heading(ui, "Window", None);
    ui.add_space(HEADING_TO_OPTIONS_PX);
    if option_line(
        ui,
        MOTION_LABEL,
        &mut launcher.config.motion,
        ControlLook::Editable,
        OptionNesting::Top,
    )
    .changed()
    {
        launcher.save_config();
    }
    match stepper_line(ui, SIZE_LABEL, launcher.config.scale.label(), true) {
        Some(StepDirection::Back) => launcher.config.scale = launcher.config.scale.smaller(),
        Some(StepDirection::Forward) => launcher.config.scale = launcher.config.scale.larger(),
        None => return,
    }
    launcher.save_config();
}
