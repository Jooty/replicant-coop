use std::sync::atomic::{AtomicBool, Ordering};

use hudhook::imgui::{MouseButton, Ui};
use nc_hook::menu_view::parts::fold_heading;
use replicant_game::kaine::{
    SPELL_BRIGHTNESS_GROUPS, SPELL_BRIGHTNESS_SLIDER_MAX, SPELL_BRIGHTNESS_SLIDER_MIN,
    SPELL_SIZE_GROUPS, SPELL_SIZE_SLIDER_MAX, SPELL_SIZE_SLIDER_MIN,
};

use crate::kaine::{
    log_spell_brightness_sliders, log_spell_size_sliders, set_spell_brightness_sliders,
    set_spell_size_sliders, spell_brightness_sliders, spell_size_sliders,
};

static SIZES_SHOWN: AtomicBool = AtomicBool::new(false);
static UNLOGGED_CHANGE: AtomicBool = AtomicBool::new(false);
const SLIDER_WIDTH_PIXELS: f32 = 190.0;

pub(super) fn draw_spell_sizes(ui: &Ui) {
    let mut shown = SIZES_SHOWN.load(Ordering::Relaxed);
    if fold_heading(ui, "spell sizes", &mut shown) {
        SIZES_SHOWN.store(shown, Ordering::Relaxed);
    }
    if !shown {
        return;
    }
    let mut sliders = spell_size_sliders();
    for group in SPELL_SIZE_GROUPS {
        let mut value = sliders.of(group);
        ui.set_next_item_width(SLIDER_WIDTH_PIXELS);
        let slid = ui
            .slider_config(
                format!("{}##spell-size", group.label()),
                SPELL_SIZE_SLIDER_MIN,
                SPELL_SIZE_SLIDER_MAX,
            )
            .display_format("x%.2f")
            .build(&mut value);
        if slid {
            sliders.set(group, value);
            set_spell_size_sliders(sliders);
            UNLOGGED_CHANGE.store(true, Ordering::Relaxed);
        }
    }
    let mut brightness = spell_brightness_sliders();
    for group in SPELL_BRIGHTNESS_GROUPS {
        let mut value = brightness.of(group);
        ui.set_next_item_width(SLIDER_WIDTH_PIXELS);
        let slid = ui
            .slider_config(
                format!("{}##spell-brightness", group.label()),
                SPELL_BRIGHTNESS_SLIDER_MIN,
                SPELL_BRIGHTNESS_SLIDER_MAX,
            )
            .display_format("x%.2f")
            .build(&mut value);
        if slid {
            brightness.set(group, value);
            set_spell_brightness_sliders(brightness);
            UNLOGGED_CHANGE.store(true, Ordering::Relaxed);
        }
    }
    if !ui.is_mouse_down(MouseButton::Left) && UNLOGGED_CHANGE.swap(false, Ordering::Relaxed) {
        log_spell_size_sliders();
        log_spell_brightness_sliders();
    }
}
