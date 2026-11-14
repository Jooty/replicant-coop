use eframe::egui::{Id, Ui};
use nc_launcher_core::{Easing, SELECTION_SWEEP_MS, Sweep, Transition};

use crate::app::motion::with_clock;

/// The selection sweep of the control known by `id`: full while `on`, empty otherwise, sweeping between the
/// two from wherever it is. A control first seen is already at rest.
pub fn sweep_toward(ui: &Ui, id: Id, on: bool) -> Sweep {
    let target = if on { 1.0 } else { 0.0 };
    with_clock(ui, |clock, data| {
        let transition = data.get_temp_mut_or_insert_with(id, || Transition::at_rest(target));
        transition.head_for(
            target,
            clock.now(),
            SELECTION_SWEEP_MS,
            Easing::EaseOut,
            clock.motion(),
        );
        clock.sweep(transition)
    })
}

/// A value of the thing known by `id` easing to `target` over `full_ms` for a whole run of 0 to 1.
pub fn ease_toward(ui: &Ui, id: Id, target: f32, full_ms: f32, easing: Easing) -> f32 {
    with_clock(ui, |clock, data| {
        let transition = data.get_temp_mut_or_insert_with(id, || Transition::at_rest(target));
        transition.head_for(target, clock.now(), full_ms, easing, clock.motion());
        clock.sample(transition)
    })
}
