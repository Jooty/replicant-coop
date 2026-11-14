pub const COPIED_HOLD_SECONDS: f32 = 1.2;

/// Whether a copy-on-click line reads COPIED `seconds` after its click; the motion clock sweeps it in and out.
pub fn copied_shown(seconds: f32) -> bool {
    (0.0..COPIED_HOLD_SECONDS).contains(&seconds)
}
