use eframe::egui::{Context, Id, Ui, util::IdTypeMap};
use nc_launcher_core::MotionClock;

const MOTION_SETTING_SALT: &str = "launcher-motion-setting";

/// Runs `read` with the motion clock at this frame's time and the context's stored motion states, then asks
/// for the next frame only when something read is still moving or waiting to.
pub fn with_clock<R>(ui: &Ui, read: impl FnOnce(&mut MotionClock, &mut IdTypeMap) -> R) -> R {
    let ctx = ui.ctx();
    let now = ui.input(|input| input.time);
    let (result, next_frame_in) = ctx.data_mut(|data| {
        let motion = data
            .get_temp::<bool>(Id::unique(MOTION_SETTING_SALT))
            .unwrap_or(true);
        let mut clock = MotionClock::new(now, motion);
        let result = read(&mut clock, data);
        (result, clock.next_frame_in())
    });
    match next_frame_in {
        Some(seconds) if seconds <= 0.0 => ctx.request_repaint(),
        Some(seconds) => ctx.request_repaint_after_secs(seconds),
        None => {}
    }
    result
}

/// The launcher's motion setting, read by every motion until it is set again.
pub fn set_motion(ctx: &Context, on: bool) {
    ctx.data_mut(|data| data.insert_temp(Id::unique(MOTION_SETTING_SALT), on));
}
