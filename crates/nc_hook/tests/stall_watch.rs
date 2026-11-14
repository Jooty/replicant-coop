use nc_hook::crash_report::{ModuleSpot, StackFrame};
use nc_hook::stall_watch::{
    GameThreadSteps, STALL_LATER_SAMPLE_EVERY_MS, STALL_MAX_SAMPLES_PER_STALL,
    STALL_SAMPLE_AFTER_MS, StallSample, StallVerdict, StallWatch, game_thread_ended_line,
    game_thread_steps, in_game_thread_step, stall_recovered_line, stall_sample_text,
};

const LAST_TICK_MS: u64 = 10_000;

fn exe(base: u64) -> ModuleSpot {
    ModuleSpot {
        name: "game.exe".to_owned(),
        base,
    }
}

#[test]
fn a_ticking_game_thread_is_never_sampled() {
    let mut watch = StallWatch::new();
    assert_eq!(watch.check(None, 50_000), StallVerdict::Ticking);
    for now in (0..20).map(|step| LAST_TICK_MS + step * 500) {
        assert_eq!(watch.check(Some(now), now + 16), StallVerdict::Ticking);
    }
}

#[test]
fn a_game_thread_that_stops_ticking_is_sampled_early_then_rarely_and_then_left_alone() {
    let mut watch = StallWatch::new();
    assert_eq!(
        watch.check(Some(LAST_TICK_MS), LAST_TICK_MS),
        StallVerdict::Ticking
    );
    let first = STALL_SAMPLE_AFTER_MS[0];
    assert_eq!(
        watch.check(Some(LAST_TICK_MS), LAST_TICK_MS + first - 1),
        StallVerdict::Ticking
    );
    let mut sampled_at = Vec::new();
    for now in (LAST_TICK_MS..LAST_TICK_MS + 1_000_000).step_by(500) {
        if let StallVerdict::Sample { stalled_ms, index } = watch.check(Some(LAST_TICK_MS), now) {
            assert_eq!(index as usize, sampled_at.len());
            sampled_at.push(stalled_ms);
        }
    }
    let last_early = STALL_SAMPLE_AFTER_MS[STALL_SAMPLE_AFTER_MS.len() - 1];
    let mut wanted = STALL_SAMPLE_AFTER_MS.to_vec();
    wanted.extend(
        (1..=u64::from(STALL_MAX_SAMPLES_PER_STALL) - STALL_SAMPLE_AFTER_MS.len() as u64)
            .map(|later| last_early + later * STALL_LATER_SAMPLE_EVERY_MS),
    );
    assert_eq!(sampled_at, wanted);
}

#[test]
fn a_long_stall_that_ends_says_so() {
    let mut watch = StallWatch::new();
    watch.check(Some(LAST_TICK_MS), LAST_TICK_MS);
    let first = STALL_SAMPLE_AFTER_MS[0];
    assert!(matches!(
        watch.check(Some(LAST_TICK_MS), LAST_TICK_MS + first),
        StallVerdict::Sample { index: 0, .. }
    ));
    assert_eq!(
        watch.check(Some(LAST_TICK_MS), LAST_TICK_MS + first + 500),
        StallVerdict::Waiting
    );
    let back = LAST_TICK_MS + first + 900;
    assert_eq!(
        watch.check(Some(back), back + 10),
        StallVerdict::Recovered {
            stalled_ms: first + 900,
            samples: 1
        }
    );
    assert_eq!(
        watch.check(Some(back + 16), back + 20),
        StallVerdict::Ticking
    );
}

#[test]
fn a_sample_names_the_instruction_the_mod_step_and_the_stack() {
    let sample = StallSample {
        thread_id: 348,
        stalled_ms: 6_012,
        index: 0,
        instruction: 0x1_4012_3456,
        instruction_module: Some(exe(0x1_4000_0000)),
        stack: vec![StackFrame {
            address: 0x1_4000_2000,
            module: exe(0x1_4000_0000),
        }],
        steps: Some(GameThreadSteps {
            current: Some("ghost tick: companion off"),
            last_finished: Some("ghost tick: companion hide"),
        }),
    };
    let text = stall_sample_text(&sample);
    assert!(text.contains("the game thread 348 has not ticked for 6012 ms (sample 0)"));
    assert!(text.contains("no exception was raised"));
    assert!(text.contains("stands at 0x0000000140123456 in game.exe rva 0x123456"));
    assert!(text.contains("inside the mod's step \"ghost tick: companion off\""));
    assert!(text.contains("stall: stack 0 0x0000000140002000 game.exe rva 0x2000"));
}

#[test]
fn a_sample_outside_the_mod_names_its_last_finished_step() {
    let sample = StallSample {
        thread_id: 1,
        stalled_ms: 12_000,
        index: 1,
        instruction: 0x7FFF_0000_1000,
        instruction_module: None,
        stack: Vec::new(),
        steps: Some(GameThreadSteps {
            current: None,
            last_finished: Some("script pop: judging a popped callback"),
        }),
    };
    let text = stall_sample_text(&sample);
    assert!(text.contains("in no loaded module"));
    assert!(text.contains(
        "in game code; the mod's last finished step was \"script pop: judging a popped callback\""
    ));
}

#[test]
fn the_end_and_recovery_lines_say_what_happened() {
    assert!(game_thread_ended_line(348, 0).contains("the game thread 348 has ended"));
    assert!(stall_recovered_line(7_000, 1).contains("ticks again after 7000 ms"));
}

#[test]
fn a_step_is_recorded_while_it_runs_and_remembered_after() {
    // the only test in this binary that writes the step record
    let inside = in_game_thread_step("test step: outer", || {
        in_game_thread_step("test step: inner", game_thread_steps)
    })
    .expect("nobody else holds the record");
    assert_eq!(inside.current, Some("test step: inner"));
    let after = game_thread_steps().expect("nobody else holds the record");
    assert_eq!(after.current, None);
    assert_eq!(after.last_finished, Some("test step: outer"));
}
