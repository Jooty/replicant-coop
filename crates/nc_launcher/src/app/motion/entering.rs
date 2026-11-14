use eframe::egui::emath::TSTransform;
use eframe::egui::{Id, Ui, vec2};
use nc_launcher_core::Easing;

use crate::app::motion::with_clock;

const ENTERING_SHIFT_PX: f32 = 8.0;
const REVEAL_SHARE_SALT: &str = "launcher-reveal-share";

/// Draws `add` coming in: from a few pixels left and faint, `delay_ms` after `since` and over `ms`, with
/// the rules inside drawing from left to right as it comes. Input is never held back.
pub fn entering<R>(
    ui: &mut Ui,
    since: f64,
    delay_ms: f32,
    ms: f32,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    let progress = with_clock(ui, |clock, _| {
        clock.entrance(since, delay_ms, ms, Easing::Linear)
    });
    if progress >= 1.0 {
        return add(ui);
    }
    let eased = Easing::EaseOut.apply(progress);
    let layer = ui.layer_id();
    let first_shape = ui
        .ctx()
        .graphics_mut(|layers| layers.entry(layer).next_idx());
    let outer_share = reveal_share(ui);
    set_reveal_share(ui, progress.min(outer_share));
    let result = ui
        .scope(|ui| {
            ui.multiply_opacity(eased);
            add(ui)
        })
        .inner;
    set_reveal_share(ui, outer_share);
    let shift = TSTransform::from_translation(vec2(-(1.0 - eased) * ENTERING_SHIFT_PX, 0.0));
    ui.ctx().graphics_mut(|layers| {
        let shapes = layers.entry(layer);
        let end = shapes.next_idx();
        shapes.transform_range(first_shape, end, shift);
    });
    result
}

/// How much of a rule is drawn where it is painted now: all of it, unless it is coming in.
pub fn reveal_share(ui: &Ui) -> f32 {
    ui.ctx()
        .data(|data| data.get_temp::<f32>(Id::unique(REVEAL_SHARE_SALT)))
        .unwrap_or(1.0)
}

fn set_reveal_share(ui: &Ui, share: f32) {
    ui.ctx()
        .data_mut(|data| data.insert_temp(Id::unique(REVEAL_SHARE_SALT), share));
}
