use replicant_game::script_runner::{SCRIPT_PAUSE_STALE_AFTER_MS, ScriptPauseWatch};

#[test]
fn a_pause_on_holds_until_the_matching_pause_off() {
    let mut watch = ScriptPauseWatch::new();
    assert!(!watch.in_force(0));
    watch.pause_on_called(100);
    assert!(watch.in_force(200));
    watch.pause_off_called();
    assert!(!watch.in_force(300));
}

#[test]
fn one_pause_off_ends_two_pause_ons_as_the_game_clears_the_whole_mask() {
    let mut watch = ScriptPauseWatch::new();
    watch.pause_on_called(100);
    watch.pause_on_called(200);
    watch.pause_off_called();
    assert!(!watch.in_force(300));
    watch.pause_on_called(400);
    assert!(watch.in_force(500));
}

#[test]
fn a_stray_pause_off_does_not_owe_a_pause_on() {
    let mut watch = ScriptPauseWatch::new();
    watch.pause_off_called();
    watch.pause_on_called(100);
    assert!(watch.in_force(150));
}

#[test]
fn a_pause_never_closed_goes_stale() {
    let mut watch = ScriptPauseWatch::new();
    watch.pause_on_called(100);
    assert!(!watch.in_force(100 + SCRIPT_PAUSE_STALE_AFTER_MS));
}
