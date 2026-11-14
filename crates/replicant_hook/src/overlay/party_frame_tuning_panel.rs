use std::sync::atomic::{AtomicBool, Ordering};

use hudhook::imgui::{MouseButton, Ui};
use nc_hook::menu_view::parts::{fold_heading, line_button};
use replicant_game::party_frame::TUNING_FIELDS;

use crate::frame_tuning::{frame_tuning, reset_frame_tuning, save_frame_tuning, set_frame_tuning};

static UNSAVED: AtomicBool = AtomicBool::new(false);
static LAYOUT_SHOWN: AtomicBool = AtomicBool::new(false);
const SLIDER_WIDTH_PIXELS: f32 = 190.0;
const NUMBER_FIELD_WIDTH_PIXELS: f32 = 80.0;

pub(super) fn draw_party_frame_tuning(ui: &Ui) {
    let mut shown = LAYOUT_SHOWN.load(Ordering::Relaxed);
    if fold_heading(ui, "party frame layout", &mut shown) {
        LAYOUT_SHOWN.store(shown, Ordering::Relaxed);
    }
    if !shown {
        return;
    }
    let mut tuning = frame_tuning();
    let mut changed = false;
    for (index, field) in TUNING_FIELDS.iter().enumerate() {
        let mut value = tuning.get(index);
        ui.set_next_item_width(SLIDER_WIDTH_PIXELS);
        let slid = ui
            .slider_config(format!("##slider-{}", field.key), field.min, field.max)
            .display_format("%.1f")
            .build(&mut value);
        ui.same_line();
        ui.set_next_item_width(NUMBER_FIELD_WIDTH_PIXELS);
        let typed = ui
            .input_float(format!("{}##number-{}", field.label, field.key), &mut value)
            .step(1.0)
            .display_format("%.1f")
            .build();
        if slid || typed {
            tuning.set(index, value);
            changed = true;
        }
    }
    if changed {
        set_frame_tuning(tuning);
    }
    ui.text("values in 1080p reference pixels:");
    ui.text(tuning.as_text());
    if line_button(ui, "reset to defaults", false, true) {
        reset_frame_tuning();
        save_frame_tuning();
    }
    if changed {
        UNSAVED.store(true, Ordering::Relaxed);
    }
    if !ui.is_mouse_down(MouseButton::Left) && UNSAVED.swap(false, Ordering::Relaxed) {
        save_frame_tuning();
    }
}
