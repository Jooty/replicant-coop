use replicant_game::kaine::{
    SwappedBodyLevel, SwappedLevelChange, SwappedLevelStep, swapped_level_line,
};
use replicant_game::prologue_levels::{CATCH_UP_SETTLE_MS, LocalLevel, PartnerLevel};

const PROLOGUE_HOST: PartnerLevel = PartnerLevel {
    level: 21,
    experience: 308_230,
};
const HOST_AFTER_THE_JUMP: PartnerLevel = PartnerLevel {
    level: 0,
    experience: 0,
};

fn her(level: i32, experience: i32) -> LocalLevel {
    LocalLevel { level, experience }
}

fn reached(host: PartnerLevel) -> LocalLevel {
    her(host.level, host.experience)
}

#[test]
fn she_is_raised_to_the_hosts_level_once_and_then_sees_it_met() {
    let mut level = SwappedBodyLevel::default();
    level.host_reported(PROLOGUE_HOST);
    assert_eq!(
        level.step(0, her(0, 0)).change,
        Some(SwappedLevelChange::Raise { gain: 308_230 })
    );
    level.change_applied(reached(PROLOGUE_HOST));
    for now in [
        CATCH_UP_SETTLE_MS,
        2 * CATCH_UP_SETTLE_MS,
        26 * CATCH_UP_SETTLE_MS,
    ] {
        assert_eq!(level.step(now, reached(PROLOGUE_HOST)).change, None);
    }
}

#[test]
fn a_save_the_old_catch_up_inflated_is_brought_down_to_the_hosts_level() {
    let mut level = SwappedBodyLevel::default();
    level.host_reported(PROLOGUE_HOST);
    assert_eq!(
        level.step(0, her(43, 6 * 308_230)).change,
        Some(SwappedLevelChange::BringDown)
    );
}

#[test]
fn experience_left_inflated_after_a_jump_reset_her_level_is_brought_down() {
    let mut level = SwappedBodyLevel::default();
    level.host_reported(HOST_AFTER_THE_JUMP);
    assert_eq!(
        level.step(0, her(0, 26 * 308_230)).change,
        Some(SwappedLevelChange::BringDown)
    );
}

#[test]
fn she_is_never_left_above_the_host() {
    let mut level = SwappedBodyLevel::default();
    level.host_reported(PROLOGUE_HOST);
    assert_eq!(
        level.step(0, her(22, 308_230)).change,
        Some(SwappedLevelChange::BringDown)
    );
    level.change_applied(reached(PROLOGUE_HOST));
    assert_eq!(
        level
            .step(CATCH_UP_SETTLE_MS, reached(PROLOGUE_HOST))
            .change,
        None
    );
}

#[test]
fn a_level_below_the_hosts_at_its_experience_is_set_to_it() {
    let mut level = SwappedBodyLevel::default();
    level.host_reported(PROLOGUE_HOST);
    assert_eq!(
        level.step(0, her(20, 308_230)).change,
        Some(SwappedLevelChange::SetToHost)
    );
}

#[test]
fn a_script_that_takes_her_levels_away_waits_for_the_hosts_next_report() {
    let mut level = SwappedBodyLevel::default();
    level.host_reported(PROLOGUE_HOST);
    level.step(0, reached(PROLOGUE_HOST));
    assert_eq!(level.step(10_000, her(0, 0)).change, None);
    assert_eq!(
        level.step(10_000 + CATCH_UP_SETTLE_MS, her(0, 0)).change,
        None
    );
    level.host_reported(HOST_AFTER_THE_JUMP);
    assert_eq!(level.step(20_000, her(0, 0)).change, None);
}

#[test]
fn a_fresh_host_report_after_the_drop_raises_her_again() {
    let mut level = SwappedBodyLevel::default();
    level.host_reported(PROLOGUE_HOST);
    level.step(0, reached(PROLOGUE_HOST));
    level.step(10_000, her(0, 0));
    level.host_reported(PROLOGUE_HOST);
    assert_eq!(
        level.step(20_000, her(0, 0)).change,
        Some(SwappedLevelChange::Raise { gain: 308_230 })
    );
}

#[test]
fn a_second_change_waits_for_the_game_to_settle() {
    let mut level = SwappedBodyLevel::default();
    level.host_reported(PROLOGUE_HOST);
    assert!(level.step(0, her(0, 0)).change.is_some());
    assert_eq!(level.step(100, her(0, 0)).change, None);
    assert!(level.step(CATCH_UP_SETTLE_MS, her(0, 0)).change.is_some());
}

#[test]
fn no_host_report_changes_nothing() {
    let mut level = SwappedBodyLevel::default();
    assert_eq!(
        level.step(0, her(43, 1_849_380)),
        SwappedLevelStep::default()
    );
}

#[test]
fn a_level_her_own_game_gives_her_is_reported_once_and_not_added_again() {
    let mut level = SwappedBodyLevel::default();
    level.host_reported(PartnerLevel {
        level: 3,
        experience: 400,
    });
    level.step(0, her(2, 380));
    level.change_applied(her(3, 400));
    let step = level.step(CATCH_UP_SETTLE_MS, her(3, 400));
    assert_eq!(step.changed_by_the_game, None);
    assert_eq!(step.change, None);
    level.host_reported(PartnerLevel {
        level: 4,
        experience: 600,
    });
    let step = level.step(2 * CATCH_UP_SETTLE_MS, her(4, 600));
    assert_eq!(step.changed_by_the_game, Some((her(3, 400), her(4, 600))));
    assert_eq!(step.change, None);
}

#[test]
fn every_change_is_one_line_with_from_to_and_why() {
    assert_eq!(
        swapped_level_line(
            SwappedLevelChange::Raise { gain: 308_230 },
            her(0, 0),
            reached(PROLOGUE_HOST),
            PROLOGUE_HOST
        ),
        "swapped body level: 0 (exp 0) -> 21 (exp 308230), host 21 (exp 308230): below the host's, +308230 experience through the game"
    );
    assert_eq!(
        swapped_level_line(
            SwappedLevelChange::BringDown,
            her(43, 1_849_380),
            reached(PROLOGUE_HOST),
            PROLOGUE_HOST
        ),
        "swapped body level: 43 (exp 1849380) -> 21 (exp 308230), host 21 (exp 308230): above the host's, brought down to it"
    );
}
