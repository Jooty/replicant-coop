use nc_hook::hud_guard::HudGuard;

#[test]
fn a_draw_that_does_not_panic_runs_every_time() {
    let mut guard = HudGuard::default();
    let mut calls = 0;
    guard.run(|| calls += 1);
    guard.run(|| calls += 1);
    assert_eq!(calls, 2);
    assert!(!guard.is_off());
}

#[test]
fn a_panic_is_caught_and_the_draw_is_skipped_from_then_on() {
    let mut guard = HudGuard::default();
    guard.run(|| panic!("made-up draw failure"));
    assert!(guard.is_off());
    let mut calls = 0;
    guard.run(|| calls += 1);
    assert_eq!(calls, 0);
}
