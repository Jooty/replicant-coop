use replicant_game::kaine::{
    GuestStandIn, HELD_TRAP_RELEASE_DELAY_MS, OwnTrapVerdict, STAND_IN_BEATS, StandInBeat,
    StandInEvent, StandInFacts, StandInSide, open_stand_in_beat, stand_in_hp_damage,
};
use replicant_proto::EnemyId;

const FIGHT: &str = "boss fight 6";
const BEAT_2_FLAGS: &[u16] = &[378, 379, 380, 381, 382, 384, 1309];
const BOSS: EnemyId = EnemyId {
    group: 100,
    ordinal: 0,
};

fn beat_with_flags(flags: &[u16]) -> Option<&'static StandInBeat> {
    open_stand_in_beat(FIGHT, |id| flags.contains(&id))
}

fn facts(side: StandInSide, flags: &[u16], now_ms: u64) -> StandInFacts {
    StandInFacts {
        side,
        in_session: true,
        fight_open: true,
        beat: beat_with_flags(flags),
        free_to_move: true,
        now_ms,
    }
}

fn moves(events: &[StandInEvent]) -> Vec<u8> {
    events
        .iter()
        .filter_map(|event| match event {
            StandInEvent::MoveGuest { beat, .. } => Some(*beat),
            _ => None,
        })
        .collect()
}

fn released(events: &[StandInEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            StandInEvent::Release(trap) => Some(trap.name.clone()),
            _ => None,
        })
        .collect()
}

fn guest_holding_trap_6035() -> GuestStandIn {
    let mut stand_in = GuestStandIn::new();
    stand_in.step(&facts(StandInSide::Guest, BEAT_2_FLAGS, 0));
    let verdict = stand_in.own_trap_fired(
        StandInSide::Guest,
        true,
        "EID_1050_a0055_Start",
        Vec::new(),
        10,
    );
    assert_eq!(verdict, OwnTrapVerdict::Hold);
    stand_in
}

fn guest_holding_trap_6034() -> GuestStandIn {
    let mut stand_in = GuestStandIn::new();
    stand_in.step(&facts(StandInSide::Guest, &[378], 0));
    let verdict = stand_in.own_trap_fired(
        StandInSide::Guest,
        true,
        "EID_1050_a0035_a0040",
        Vec::new(),
        10,
    );
    assert_eq!(verdict, OwnTrapVerdict::Hold);
    stand_in
}

#[test]
fn each_beat_opens_from_the_host_flags_and_closes_when_the_host_catches_up() {
    assert_eq!(beat_with_flags(&[]), None);
    assert_eq!(beat_with_flags(&[377, 378, 1307]).map(|b| b.beat), Some(1));
    assert_eq!(beat_with_flags(&[378, 379]), None);
    assert_eq!(beat_with_flags(&[378, 379, 380]).map(|b| b.beat), Some(2));
    // between the two "on the way" flags of beat 2 (its scene) the beat stays open
    assert_eq!(
        beat_with_flags(&[378, 379, 380, 381]).map(|b| b.beat),
        Some(2)
    );
    assert_eq!(beat_with_flags(&[378, 379, 380, 381, 382, 383]), None);
    assert_eq!(open_stand_in_beat("boss fight 3", |_| true), None);
}

#[test]
fn the_guest_is_moved_to_spot_one_after_its_own_scene_end() {
    let mut stand_in = GuestStandIn::new();
    stand_in.function_ran("EID_1050_a0030_End");
    let events = stand_in.step(&facts(StandInSide::Guest, &[378], 0));
    assert!(events.contains(&StandInEvent::BeatOpened { beat: 1 }));
    assert!(events.contains(&StandInEvent::MoveGuest {
        beat: 1,
        spot: STAND_IN_BEATS[0].spot,
        yaw_degrees: 90.0,
    }));
    assert!(moves(&stand_in.step(&facts(StandInSide::Guest, &[378], 16))).is_empty());
}

#[test]
fn the_guest_is_moved_to_spot_two_only_after_its_own_move_end() {
    let mut stand_in = GuestStandIn::new();
    stand_in.function_ran("EID_1050_a0040_End");
    assert!(moves(&stand_in.step(&facts(StandInSide::Guest, &[378, 379, 380], 0))).is_empty());
    stand_in.function_ran("EID_1050_a0040_Move_End");
    assert_eq!(
        moves(&stand_in.step(&facts(StandInSide::Guest, &[378, 379, 380], 16))),
        vec![2]
    );
}

#[test]
fn the_guest_is_moved_back_to_spot_two_after_the_scene_inside_the_beat() {
    let mut stand_in = GuestStandIn::new();
    stand_in.function_ran("EID_1050_a0040_Move_End");
    stand_in.step(&facts(StandInSide::Guest, &[378, 379, 380], 0));
    stand_in.function_ran("EID_1050_a0050_01_End");
    assert_eq!(
        moves(&stand_in.step(&facts(StandInSide::Guest, &[378, 379, 380, 381, 382], 16))),
        vec![2]
    );
}

#[test]
fn the_move_waits_until_the_guest_is_free_to_move() {
    let mut stand_in = GuestStandIn::new();
    stand_in.function_ran("EID_1050_a0030_End");
    let busy = StandInFacts {
        free_to_move: false,
        ..facts(StandInSide::Guest, &[378], 0)
    };
    assert!(moves(&stand_in.step(&busy)).is_empty());
    assert_eq!(
        moves(&stand_in.step(&facts(StandInSide::Guest, &[378], 16))),
        vec![1]
    );
}

#[test]
fn the_host_is_never_moved() {
    let mut stand_in = GuestStandIn::new();
    stand_in.function_ran("EID_1050_a0030_End");
    assert!(moves(&stand_in.step(&facts(StandInSide::Host, &[378], 0))).is_empty());
}

#[test]
fn a_retry_rebuilds_the_beat_from_flags_and_moves_the_guest_again() {
    let mut stand_in = guest_holding_trap_6034();
    stand_in.function_ran("EID_1050_a0035g_01_Continue");
    assert!(stand_in.held_traps().is_empty());
    let events = stand_in.step(&facts(StandInSide::Guest, &[378], 20));
    assert!(events.iter().any(|event| matches!(
        event,
        StandInEvent::Dropped { trap, .. } if trap.name == "EID_1050_a0035_a0040"
    )));
    assert_eq!(moves(&events), vec![1]);
    assert_eq!(stand_in.open_beat().map(|beat| beat.beat), Some(1));
}

#[test]
fn the_guest_holds_its_own_catch_up_trap_while_a_beat_is_open() {
    let mut stand_in = guest_holding_trap_6034();
    assert_eq!(stand_in.held_traps().len(), 1);
    assert!(released(&stand_in.step(&facts(StandInSide::Guest, &[378], 60_000))).is_empty());
    let report = stand_in.take_report().expect("beat open");
    assert_eq!(report.beat.beat, 1);
    assert_eq!(report.held, vec!["EID_1050_a0035_a0040".to_owned()]);
    assert_eq!(report.holds, 1);
}

#[test]
fn a_held_trap_runs_after_the_host_fired_it() {
    let mut stand_in = guest_holding_trap_6035();
    stand_in.host_fired("EID_1050_a0055_Start", 1_000);
    assert!(released(&stand_in.step(&facts(StandInSide::Guest, BEAT_2_FLAGS, 1_001))).is_empty());
    assert_eq!(
        released(&stand_in.step(&facts(
            StandInSide::Guest,
            BEAT_2_FLAGS,
            1_000 + HELD_TRAP_RELEASE_DELAY_MS
        ))),
        vec!["EID_1050_a0055_Start".to_owned()]
    );
}

#[test]
fn a_held_trap_runs_a_little_after_the_host_catches_up() {
    let mut stand_in = guest_holding_trap_6035();
    let caught_up = [378, 379, 380, 383];
    let closed = stand_in.step(&facts(StandInSide::Guest, &caught_up, 2_000));
    assert!(closed.contains(&StandInEvent::BeatClosed {
        beat: 2,
        catch_up_seen: false
    }));
    assert!(released(&closed).is_empty());
    assert_eq!(
        released(&stand_in.step(&facts(
            StandInSide::Guest,
            &caught_up,
            2_000 + HELD_TRAP_RELEASE_DELAY_MS
        ))),
        vec!["EID_1050_a0055_Start".to_owned()]
    );
}

#[test]
fn a_beat_closed_after_the_host_ran_its_catch_up_trap_says_so() {
    let mut stand_in = guest_holding_trap_6034();
    stand_in.host_fired("EID_1050_a0035_a0040", 1_000);
    let closed = stand_in.step(&facts(StandInSide::Guest, &[378, 379], 1_010));
    assert!(closed.contains(&StandInEvent::BeatClosed {
        beat: 1,
        catch_up_seen: true
    }));
}

#[test]
fn a_beat_the_host_ran_on_the_host_is_caught_up_there() {
    let mut stand_in = GuestStandIn::new();
    stand_in.step(&facts(StandInSide::Host, &[378], 0));
    stand_in.function_ran("EID_1050_a0035_a0040");
    let closed = stand_in.step(&facts(StandInSide::Host, &[378, 379], 10));
    assert!(closed.contains(&StandInEvent::BeatClosed {
        beat: 1,
        catch_up_seen: true
    }));
}

#[test]
fn a_story_jump_past_the_fight_closes_the_beat_unseen_and_drops_its_held_traps() {
    let mut stand_in = guest_holding_trap_6035();
    let jumped = StandInFacts {
        fight_open: false,
        ..facts(StandInSide::Guest, &[378, 379, 380, 383, 386], 125_000)
    };
    let events = stand_in.step(&jumped);
    assert!(events.contains(&StandInEvent::BeatClosed {
        beat: 2,
        catch_up_seen: false
    }));
    assert!(released(&events).is_empty());
    assert!(events.iter().any(|event| matches!(
        event,
        StandInEvent::Dropped { trap, .. } if trap.name == "EID_1050_a0055_Start"
    )));
    assert!(
        released(&stand_in.step(&StandInFacts {
            now_ms: 125_000 + HELD_TRAP_RELEASE_DELAY_MS,
            ..jumped
        }))
        .is_empty()
    );
}

#[test]
fn a_held_trap_whose_host_copy_is_sent_here_is_never_run_from_the_guest() {
    let mut stand_in = guest_holding_trap_6034();
    stand_in.host_fired("EID_1050_a0035_a0040", 1_000);
    let events = stand_in.step(&facts(
        StandInSide::Guest,
        &[378, 379],
        1_000 + HELD_TRAP_RELEASE_DELAY_MS,
    ));
    assert!(released(&events).is_empty());
    assert!(events.iter().any(|event| matches!(
        event,
        StandInEvent::Dropped { trap, .. } if trap.name == "EID_1050_a0035_a0040"
    )));
    assert!(stand_in.held_traps().is_empty());
}

#[test]
fn a_held_trap_is_dropped_when_the_hosts_copy_runs_here() {
    let mut stand_in = guest_holding_trap_6034();
    stand_in.host_fired("EID_1050_a0035_a0040", 1_000);
    stand_in.function_ran("EID_1050_a0035_a0040");
    let events = stand_in.step(&facts(StandInSide::Guest, &[378, 379], 60_000));
    assert!(released(&events).is_empty());
    assert!(stand_in.held_traps().is_empty());
}

#[test]
fn held_traps_are_dropped_when_the_fight_window_closes() {
    let mut stand_in = guest_holding_trap_6034();
    let closed = StandInFacts {
        fight_open: false,
        ..facts(StandInSide::Guest, &[378], 100)
    };
    let events = stand_in.step(&closed);
    assert!(released(&events).is_empty());
    assert!(stand_in.held_traps().is_empty());
    assert_eq!(stand_in.open_beat(), None);
}

#[test]
fn the_host_runs_its_trap_and_tells_the_guest() {
    let mut stand_in = GuestStandIn::new();
    stand_in.step(&facts(StandInSide::Host, &[378, 379, 380], 0));
    for trap in [
        "Put_Enemy_AL3_090",
        "FLAG_DELETE_TABLE03",
        "EID_1050_a0055_Start",
    ] {
        assert_eq!(
            stand_in.own_trap_fired(StandInSide::Host, true, trap, Vec::new(), 5),
            OwnTrapVerdict::RunAndTellTheGuest
        );
    }
    assert!(stand_in.held_traps().is_empty());
}

#[test]
fn traps_run_as_usual_outside_a_beat() {
    let mut stand_in = GuestStandIn::new();
    stand_in.step(&facts(StandInSide::Guest, &[378, 379], 0));
    assert_eq!(
        stand_in.own_trap_fired(
            StandInSide::Guest,
            true,
            "EID_1050_a0035_a0040",
            Vec::new(),
            5
        ),
        OwnTrapVerdict::Run
    );
    stand_in.step(&facts(StandInSide::Guest, &[378], 10));
    assert_eq!(
        stand_in.own_trap_fired(
            StandInSide::Guest,
            true,
            "Put_Enemy_AL3_090",
            Vec::new(),
            15
        ),
        OwnTrapVerdict::Run
    );
}

#[test]
fn no_session_means_no_beat_no_hold_and_held_traps_run() {
    let mut stand_in = guest_holding_trap_6034();
    let alone = StandInFacts {
        in_session: false,
        ..facts(StandInSide::Guest, &[378], 50)
    };
    assert_eq!(
        released(&stand_in.step(&alone)),
        vec!["EID_1050_a0035_a0040".to_owned()]
    );
    assert_eq!(stand_in.open_beat(), None);
    assert_eq!(
        stand_in.own_trap_fired(
            StandInSide::Guest,
            false,
            "EID_1050_a0035_a0040",
            Vec::new(),
            60
        ),
        OwnTrapVerdict::Run
    );
}

#[test]
fn a_guest_hit_on_the_boss_in_a_beat_is_diminished_but_never_to_nothing() {
    assert!(stand_in_hp_damage(100) < 100);
    assert_eq!(stand_in_hp_damage(1), 1);
    assert_eq!(stand_in_hp_damage(0), 0);
    let mut stand_in = GuestStandIn::new();
    assert_eq!(stand_in.scales_hits_on(BOSS), None);
    stand_in.step(&facts(StandInSide::Host, &[378], 0));
    assert_eq!(stand_in.scales_hits_on(BOSS).map(|beat| beat.beat), Some(1));
    assert_eq!(
        stand_in.scales_hits_on(EnemyId {
            group: 39,
            ordinal: 0
        }),
        None
    );
}
