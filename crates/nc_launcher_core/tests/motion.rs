use nc_launcher_core::{
    Easing, MotionClock, SECTION_ENTER_MS, SELECTION_SWEEP_MS, Sweep, Transition, drift_offset_px,
};

const SWEEP_SECONDS: f64 = SELECTION_SWEEP_MS as f64 / 1000.0;

fn swept_on_at(start: f64) -> Transition {
    let mut sweep = Transition::at_rest(0.0);
    sweep.head_for(1.0, start, SELECTION_SWEEP_MS, Easing::EaseOut, true);
    sweep
}

#[test]
fn a_sweep_ends_exactly_full() {
    let sweep = swept_on_at(10.0);
    let mut clock = MotionClock::new(10.0 + SWEEP_SECONDS, true);
    assert_eq!(clock.sample(&sweep), 1.0);
    let mut later = MotionClock::new(99.0, true);
    assert_eq!(later.sweep(&sweep), Sweep::FULL);
    assert_eq!(later.next_frame_in(), None);
}

#[test]
fn a_sweep_rises_without_overshooting() {
    let sweep = swept_on_at(0.0);
    let mut last = 0.0;
    for step in 0..=24 {
        let value = sweep.value_at(f64::from(step) * SWEEP_SECONDS / 20.0);
        assert!(value >= last && value <= 1.0, "step {step}: {value}");
        last = value;
    }
}

#[test]
fn an_interrupted_sweep_starts_from_where_it_was() {
    let mut sweep = swept_on_at(0.0);
    let halfway = SWEEP_SECONDS / 2.0;
    let reached = sweep.value_at(halfway);
    assert!(reached > 0.0 && reached < 1.0);
    sweep.head_for(0.0, halfway, SELECTION_SWEEP_MS, Easing::EaseOut, true);
    assert_eq!(sweep.value_at(halfway), reached);
    assert!(!sweep.rising());
    let back_by = halfway + SWEEP_SECONDS * f64::from(reached);
    assert_eq!(sweep.value_at(back_by), 0.0);
    assert!(sweep.value_at(back_by - 0.001) > 0.0);
}

#[test]
fn no_motion_means_the_end_state_at_once() {
    let mut sweep = Transition::at_rest(0.0);
    sweep.head_for(1.0, 5.0, SELECTION_SWEEP_MS, Easing::EaseOut, false);
    assert_eq!(sweep.value_at(5.0), 1.0);
    let running = swept_on_at(5.0);
    let mut clock = MotionClock::new(5.0, false);
    assert_eq!(clock.sample(&running), 1.0);
    assert_eq!(
        clock.entrance(5.0, 40.0, SECTION_ENTER_MS, Easing::EaseOut),
        1.0
    );
    assert_eq!(clock.drift_px(250.0, 8.0), 0.0);
    assert_eq!(clock.next_frame_in(), None);
}

#[test]
fn an_idle_page_asks_for_no_repaint() {
    let mut clock = MotionClock::new(30.0, true);
    clock.sample(&Transition::at_rest(1.0));
    clock.sample(&swept_on_at(1.0));
    clock.entrance(f64::NEG_INFINITY, 0.0, SECTION_ENTER_MS, Easing::EaseOut);
    clock.entrance(2.0, 40.0, SECTION_ENTER_MS, Easing::EaseOut);
    assert_eq!(clock.next_frame_in(), None);
}

#[test]
fn a_running_motion_asks_for_the_next_frame_until_it_ends() {
    let mut clock = MotionClock::new(0.05, true);
    clock.sample(&swept_on_at(0.0));
    assert_eq!(clock.next_frame_in(), Some(0.0));
    let mut waiting = MotionClock::new(1.0, true);
    assert_eq!(
        waiting.entrance(1.0, 80.0, SECTION_ENTER_MS, Easing::EaseOut),
        0.0
    );
    assert_eq!(waiting.next_frame_in(), Some(0.0));
}

#[test]
fn an_entrance_waits_for_its_delay_then_ends_exactly_in() {
    let mut clock = MotionClock::new(1.04, true);
    assert_eq!(
        clock.entrance(1.0, 40.0, SECTION_ENTER_MS, Easing::Linear),
        0.0
    );
    let mut end = MotionClock::new(1.0 + 0.04 + 0.18, true);
    assert_eq!(
        end.entrance(1.0, 40.0, SECTION_ENTER_MS, Easing::Linear),
        1.0
    );
}

#[test]
fn each_easing_starts_at_zero_and_ends_at_one() {
    for easing in [Easing::Linear, Easing::EaseOut] {
        assert_eq!(easing.apply(0.0), 0.0);
        assert_eq!(easing.apply(1.0), 1.0);
        assert_eq!(easing.apply(-1.0), 0.0);
        assert_eq!(easing.apply(2.0), 1.0);
    }
    assert_eq!(Easing::Linear.apply(0.25), 0.25);
    assert!(Easing::EaseOut.apply(0.25) > 0.25);
}

#[test]
fn a_filling_sweep_grows_from_the_left_and_an_emptying_one_leaves_from_the_left() {
    let filling = Sweep {
        amount: 0.25,
        filling: true,
    };
    assert_eq!(filling.covered(0.0, 100.0), (0.0, 25.0));
    let emptying = Sweep {
        amount: 0.25,
        filling: false,
    };
    assert_eq!(emptying.covered(0.0, 100.0), (75.0, 100.0));
}

#[test]
fn a_drift_moves_one_pixel_per_step_and_wraps() {
    assert_eq!(drift_offset_px(0.0, 250.0, 8.0), 0.0);
    assert_eq!(drift_offset_px(0.26, 250.0, 8.0), 1.0);
    assert_eq!(drift_offset_px(2.1, 250.0, 8.0), 0.0);
    let mut clock = MotionClock::new(0.1, true);
    clock.drift_px(250.0, 8.0);
    let next = clock
        .next_frame_in()
        .expect("a drift asks for its next pixel");
    assert!((next - 0.15).abs() < 1e-4, "{next}");
}
