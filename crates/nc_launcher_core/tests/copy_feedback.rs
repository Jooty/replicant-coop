use nc_launcher_core::{COPIED_HOLD_SECONDS, copied_shown};

#[test]
fn the_copied_state_holds_then_goes() {
    assert!(copied_shown(0.0));
    assert!(copied_shown(COPIED_HOLD_SECONDS - 0.01));
    assert!(!copied_shown(COPIED_HOLD_SECONDS));
}

#[test]
fn nothing_shows_before_a_click() {
    assert!(!copied_shown(-1.0));
}
