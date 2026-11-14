use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Mutex, PoisonError};

use hudhook::imgui::Ui;
use nc_hook::menu_view::parts::{fold_heading, line_button, switch_line};
use replicant_game::Replicant;
use replicant_game::minimap::{
    MINIMAP_CENTER_X_RANGE, MINIMAP_CENTER_Y_RANGE, MINIMAP_DOT_SIZE_RANGE, MINIMAP_RADIUS_RANGE,
    MINIMAP_REFERENCE_HEIGHT_PIXELS, MINIMAP_WORLD_RANGE_RANGE, MinimapTuning, minimap_dot,
};

use crate::diagnostics::log_line;
use crate::memory::InProcessMemory;
use crate::minimap::{
    minimap_shown_recently, partner_icon_index, real_marker_active, set_partner_icon_index,
};
use crate::net;
use crate::overlay::partner_marker::shown_partner;

const DOT_COLOR: [f32; 4] = [1.0, 0.85, 0.25, 0.95];
const DOT_OUTLINE_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.9];
const DOT_OUTLINE_PIXELS: f32 = 1.5;
const SLIDER_WIDTH_PIXELS: f32 = 190.0;
const STATE_UNKNOWN: u8 = 0;
const STATE_HIDDEN: u8 = 1;
const STATE_INSIDE: u8 = 2;
const STATE_ON_EDGE: u8 = 3;

static TUNING: Mutex<MinimapTuning> = Mutex::new(MinimapTuning::new());
static FALLBACK_FORCED: AtomicBool = AtomicBool::new(false);
static TUNING_SHOWN: AtomicBool = AtomicBool::new(false);
static LAST_LOGGED_STATE: AtomicU8 = AtomicU8::new(STATE_UNKNOWN);

pub(super) fn draw_minimap_dot(ui: &Ui, game: Option<&Replicant<InProcessMemory>>) {
    let tuning = *TUNING.lock().unwrap_or_else(PoisonError::into_inner);
    let wanted = tuning.enabled
        && FALLBACK_FORCED.load(Ordering::Relaxed)
        && (!real_marker_active() || minimap_shown_recently());
    if !wanted {
        return;
    }
    let Some((offset, clamped, center, size)) = compute_dot(ui, game, &tuning) else {
        log_state_change(STATE_HIDDEN, [0.0, 0.0]);
        return;
    };
    let state = if clamped { STATE_ON_EDGE } else { STATE_INSIDE };
    log_state_change(state, offset);
    let at = [center[0] + offset[0], center[1] + offset[1]];
    let draw_list = ui.get_background_draw_list();
    draw_list
        .add_circle(at, size + DOT_OUTLINE_PIXELS, DOT_OUTLINE_COLOR)
        .filled(true)
        .build();
    draw_list
        .add_circle(at, size, DOT_COLOR)
        .filled(true)
        .build();
}

pub(super) fn draw_minimap_tuning(ui: &Ui) {
    let mut shown = TUNING_SHOWN.load(Ordering::Relaxed);
    if fold_heading(ui, "minimap dot", &mut shown) {
        TUNING_SHOWN.store(shown, Ordering::Relaxed);
    }
    if !shown {
        return;
    }
    let mut tuning = *TUNING.lock().unwrap_or_else(PoisonError::into_inner);
    let mut forced = FALLBACK_FORCED.load(Ordering::Relaxed);
    if switch_line(
        ui,
        "developer: also draw the overlay dot (fallback)",
        &mut forced,
    ) {
        FALLBACK_FORCED.store(forced, Ordering::Relaxed);
    }
    let mut index = partner_icon_index();
    ui.set_next_item_width(SLIDER_WIDTH_PIXELS);
    if ui.slider("partner icon index (game friend icons)", 0, 7, &mut index) {
        set_partner_icon_index(index);
    }
    ui.text(if real_marker_active() {
        "game minimap icon: active"
    } else {
        "game minimap icon: off, the overlay dot is the fallback"
    });
    let mut changed = switch_line(ui, "overlay dot allowed", &mut tuning.enabled);
    changed |= switch_line(
        ui,
        "minimap turns with the camera",
        &mut tuning.rotate_with_camera,
    );
    changed |= slider(ui, "centre X", &mut tuning.center_x, MINIMAP_CENTER_X_RANGE);
    changed |= slider(ui, "centre Y", &mut tuning.center_y, MINIMAP_CENTER_Y_RANGE);
    changed |= slider(
        ui,
        "minimap radius",
        &mut tuning.radius,
        MINIMAP_RADIUS_RANGE,
    );
    changed |= slider(
        ui,
        "world units to the rim",
        &mut tuning.world_range,
        MINIMAP_WORLD_RANGE_RANGE,
    );
    changed |= slider(ui, "dot size", &mut tuning.dot_size, MINIMAP_DOT_SIZE_RANGE);
    if line_button(ui, "reset minimap dot", false, true) {
        tuning = MinimapTuning::new();
        changed = true;
    }
    ui.text("values in 1080p reference pixels");
    if changed {
        *TUNING.lock().unwrap_or_else(PoisonError::into_inner) = tuning;
    }
}

fn slider(ui: &Ui, label: &str, value: &mut f32, range: (f32, f32)) -> bool {
    ui.set_next_item_width(SLIDER_WIDTH_PIXELS);
    ui.slider_config(label, range.0, range.1)
        .display_format("%.0f")
        .build(value)
}

fn compute_dot(
    ui: &Ui,
    game: Option<&Replicant<InProcessMemory>>,
    tuning: &MinimapTuning,
) -> Option<([f32; 2], bool, [f32; 2], f32)> {
    let game = game?;
    let net = net::handle()?;
    let view = net.view();
    let shown = shown_partner(&view, game)?;
    let local = view.local.as_ref()?.position;
    let dot = minimap_dot(local, shown.position, &shown.camera, tuning)?;
    let [width, height] = ui.io().display_size;
    let scale = height / MINIMAP_REFERENCE_HEIGHT_PIXELS;
    Some((
        [dot.offset_x * scale, dot.offset_y * scale],
        dot.clamped_to_edge,
        [
            width - (MINIMAP_CENTER_X_RANGE.1 - tuning.center_x) * scale,
            tuning.center_y * scale,
        ],
        tuning.dot_size * scale,
    ))
}

fn log_state_change(state: u8, offset: [f32; 2]) {
    if LAST_LOGGED_STATE.swap(state, Ordering::Relaxed) == state {
        return;
    }
    let name = match state {
        STATE_HIDDEN => "hidden",
        STATE_INSIDE => "inside the rim",
        _ => "on the rim",
    };
    log_line(&format!(
        "minimap dot: {name} (offset {:.0},{:.0} px)",
        offset[0], offset[1]
    ));
}
