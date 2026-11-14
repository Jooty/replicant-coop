use replicant_game::prologue_levels::{
    CATCH_UP_SETTLE_MS, LEVEL_RESEND_INTERVAL_MS, LevelCatchUp, LocalLevel, PROLOGUE_PHASES,
    PartnerLevel,
};

const PROLOGUE: &str = PROLOGUE_PHASES[0];
const VILLAGE: &str = "A_CENTER_VILLAGE_01";

fn local(level: i32, experience: i32) -> LocalLevel {
    LocalLevel { level, experience }
}

fn partner(level: i32, experience: i32) -> PartnerLevel {
    PartnerLevel { level, experience }
}

#[test]
fn the_first_prologue_step_sends_the_local_level() {
    let mut sync = LevelCatchUp::default();
    let step = sync.step(0, PROLOGUE, local(3, 40));
    assert_eq!(step.send, Some(local(3, 40)));
    assert_eq!(step.gain, None);
}

#[test]
fn the_level_is_sent_again_only_on_change_or_after_the_interval() {
    let mut sync = LevelCatchUp::default();
    sync.step(0, PROLOGUE, local(3, 40));
    assert_eq!(sync.step(100, PROLOGUE, local(3, 40)).send, None);
    assert_eq!(
        sync.step(200, PROLOGUE, local(3, 55)).send,
        Some(local(3, 55))
    );
    assert_eq!(sync.step(300, PROLOGUE, local(3, 55)).send, None);
    assert_eq!(
        sync.step(200 + LEVEL_RESEND_INTERVAL_MS, PROLOGUE, local(3, 55))
            .send,
        Some(local(3, 55))
    );
}

#[test]
fn the_lower_game_gains_the_missing_experience() {
    let mut sync = LevelCatchUp::default();
    sync.partner_reported(partner(5, 300));
    let step = sync.step(0, PROLOGUE, local(3, 120));
    assert_eq!(step.gain, Some(180));
}

#[test]
fn the_higher_or_equal_game_gains_nothing() {
    let mut sync = LevelCatchUp::default();
    sync.partner_reported(partner(3, 120));
    assert_eq!(sync.step(0, PROLOGUE, local(5, 300)).gain, None);
    assert_eq!(sync.step(10, PROLOGUE, local(3, 120)).gain, None);
}

#[test]
fn no_partner_report_means_no_gain() {
    let mut sync = LevelCatchUp::default();
    assert_eq!(sync.step(0, PROLOGUE, local(1, 0)).gain, None);
}

#[test]
fn a_second_gain_waits_for_the_game_to_settle() {
    let mut sync = LevelCatchUp::default();
    sync.partner_reported(partner(5, 300));
    assert_eq!(sync.step(0, PROLOGUE, local(3, 120)).gain, Some(180));
    assert_eq!(sync.step(100, PROLOGUE, local(3, 120)).gain, None);
    assert_eq!(
        sync.step(CATCH_UP_SETTLE_MS, PROLOGUE, local(3, 120)).gain,
        Some(180)
    );
}

#[test]
fn a_later_higher_report_raises_the_target() {
    let mut sync = LevelCatchUp::default();
    sync.partner_reported(partner(5, 300));
    sync.step(0, PROLOGUE, local(5, 300));
    sync.partner_reported(partner(6, 450));
    assert_eq!(sync.step(10_000, PROLOGUE, local(5, 300)).gain, Some(150));
}

#[test]
fn outside_the_prologue_the_level_is_sent_but_nothing_is_gained() {
    let mut sync = LevelCatchUp::default();
    sync.partner_reported(partner(9, 900));
    let step = sync.step(0, VILLAGE, local(1, 0));
    assert_eq!(step.send, Some(local(1, 0)));
    assert_eq!(step.gain, None);
}

#[test]
fn a_level_change_is_sent_at_once_in_any_phase() {
    let mut sync = LevelCatchUp::default();
    sync.step(0, PROLOGUE, local(21, 308_230));
    assert_eq!(sync.step(10, VILLAGE, local(21, 308_230)).send, None);
    assert_eq!(sync.step(20, VILLAGE, local(0, 0)).send, Some(local(0, 0)));
}

#[test]
fn only_the_first_town_phase_is_a_prologue_phase() {
    assert_eq!(PROLOGUE_PHASES, ["A_TOKYO_FIELD_01"]);
}

#[test]
fn a_script_that_takes_the_levels_away_is_not_undone_by_the_partners_old_report() {
    let mut sync = LevelCatchUp::default();
    sync.partner_reported(partner(21, 308_230));
    sync.step(0, PROLOGUE, local(21, 308_230));
    assert_eq!(sync.step(10_000, PROLOGUE, local(0, 0)).gain, None);
    assert_eq!(
        sync.step(10_000 + CATCH_UP_SETTLE_MS, PROLOGUE, local(0, 0))
            .gain,
        None
    );
}

#[test]
fn the_catch_up_works_again_on_the_next_prologue_visit() {
    let mut sync = LevelCatchUp::default();
    sync.partner_reported(partner(21, 308_230));
    sync.step(0, PROLOGUE, local(21, 308_230));
    sync.step(10, PROLOGUE, local(0, 0));
    sync.step(20, VILLAGE, local(0, 0));
    assert_eq!(sync.step(30, PROLOGUE, local(0, 0)).gain, Some(308_230));
}
