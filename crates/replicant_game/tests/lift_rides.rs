use replicant_game::lift_rides::{
    GUEST_LIFT_ANSWER_WAIT_MS, GUEST_LIFT_ARRIVAL_WATCH_MS, GuestCallStep, GuestLiftCall,
    GuestRideEvent, GuestRideFacts, HOST_LIFT_BUSY_WAIT_MS, HOST_LIFT_PLACE_SETTLE_MS,
    HOST_LIFT_RIDE_WATCH_MS, HostBusy, HostLiftRide, HostRideFacts, HostRideStep, LIFT_PHASES,
    RideEnd, lift_ride, warp_pick_origin,
};
use replicant_proto::{LiftCall, LiftCallOutcome};

const LIFT_PHASE: u16 = 12;
const PANEL: [f32; 3] = [19.5, 0.0, -2.5];

fn a_call() -> LiftCall {
    LiftCall {
        phase: LIFT_PHASE,
        point: 401,
        origin: 400,
        position: PANEL,
    }
}

fn free_host(now_ms: u64) -> HostRideFacts {
    HostRideFacts {
        now_ms,
        phase: Some(LIFT_PHASE),
        busy: None,
        menu_open: false,
        script_pause: false,
    }
}

fn received() -> HostLiftRide {
    let mut ride = HostLiftRide::new();
    assert_eq!(
        ride.receive(a_call(), Some((LIFT_PHASE, "A_MOUNT_ROBOT_01")), 0),
        Ok(())
    );
    ride
}

#[test]
fn a_lift_pick_in_a_lift_map_names_the_chain_the_game_starts() {
    let ride = lift_ride("A_MOUNT_ROBOT_01", 401, 400).expect("a lift ride");
    assert_eq!(ride.chain, "GM_Elevator_07_01");
    for phase in LIFT_PHASES {
        assert!(lift_ride(phase, 301, 300).is_some(), "{phase}");
    }
    assert_eq!(
        lift_ride("B_MOUNT_ROBOT_10", 500, 501).map(|r| r.chain),
        Some("GM_Elevator_10_01")
    );
}

#[test]
fn other_travel_menus_cancels_and_unknown_pairs_are_not_lifts() {
    assert_eq!(lift_ride("B_DESERT_FIELD_01", 401, 400), None);
    assert_eq!(lift_ride("A_MOUNT_ROBOT_01", -1, 400), None);
    assert_eq!(lift_ride("A_MOUNT_ROBOT_01", 400, 400), None);
    assert_eq!(lift_ride("A_MOUNT_ROBOT_01", 701, 700), None);
}

#[test]
fn a_panel_pick_takes_its_origin_from_the_open_warp_window() {
    assert_eq!(warp_pick_origin(-1, 300), 300);
    assert_eq!(warp_pick_origin(401, 300), 401);
}

#[test]
fn the_guest_sends_one_call_and_drops_a_second_while_it_is_in_flight() {
    let mut call = GuestLiftCall::new();
    assert_eq!(call.call(0, LIFT_PHASE, 401), GuestCallStep::Send);
    assert_eq!(
        call.call(100, LIFT_PHASE, 301),
        GuestCallStep::DroppedOneInFlight
    );
    assert!(call.in_flight());
}

#[test]
fn a_refusal_ends_the_guest_call_and_frees_it_for_the_next_pick() {
    let mut call = GuestLiftCall::new();
    call.call(0, LIFT_PHASE, 401);
    assert_eq!(
        call.answered(50, 401, LiftCallOutcome::HostBusy),
        Some(GuestRideEvent::Refused(LiftCallOutcome::HostBusy))
    );
    assert!(!call.in_flight());
    assert_eq!(call.call(60, LIFT_PHASE, 401), GuestCallStep::Send);
}

#[test]
fn an_answer_for_another_floor_is_ignored() {
    let mut call = GuestLiftCall::new();
    call.call(0, LIFT_PHASE, 401);
    assert_eq!(call.answered(50, 301, LiftCallOutcome::Riding), None);
    assert!(call.in_flight());
}

#[test]
fn the_guest_gives_up_when_the_host_never_answers() {
    let mut call = GuestLiftCall::new();
    call.call(0, LIFT_PHASE, 401);
    let facts = |now_ms| GuestRideFacts {
        now_ms,
        phase: Some(LIFT_PHASE),
        moved_to_the_host_just_now: false,
    };
    assert_eq!(call.tick(&facts(GUEST_LIFT_ANSWER_WAIT_MS - 1)), None);
    assert_eq!(
        call.tick(&facts(GUEST_LIFT_ANSWER_WAIT_MS)),
        Some(GuestRideEvent::NoAnswer {
            waited_ms: GUEST_LIFT_ANSWER_WAIT_MS
        })
    );
    assert!(!call.in_flight());
}

#[test]
fn the_guest_arrives_by_a_pull_or_by_the_host_map_change() {
    let mut pulled = GuestLiftCall::new();
    pulled.call(0, LIFT_PHASE, 401);
    pulled.answered(100, 401, LiftCallOutcome::Riding);
    let loading = GuestRideFacts {
        now_ms: 2_000,
        phase: None,
        moved_to_the_host_just_now: false,
    };
    assert_eq!(pulled.tick(&loading), None);
    let pull = GuestRideFacts {
        moved_to_the_host_just_now: true,
        phase: Some(LIFT_PHASE),
        ..loading
    };
    assert_eq!(
        pulled.tick(&pull),
        Some(GuestRideEvent::BroughtAlong { waited_ms: 1_900 })
    );

    let mut followed = GuestLiftCall::new();
    followed.call(0, LIFT_PHASE, 301);
    followed.answered(100, 301, LiftCallOutcome::Riding);
    let new_map = GuestRideFacts {
        phase: Some(LIFT_PHASE + 1),
        ..loading
    };
    assert_eq!(
        followed.tick(&new_map),
        Some(GuestRideEvent::ArrivedInNewMap { waited_ms: 1_900 })
    );
}

#[test]
fn the_guest_stops_watching_a_ride_that_never_reaches_it() {
    let mut call = GuestLiftCall::new();
    call.call(0, LIFT_PHASE, 401);
    call.answered(0, 401, LiftCallOutcome::Riding);
    let facts = GuestRideFacts {
        now_ms: GUEST_LIFT_ARRIVAL_WATCH_MS,
        phase: Some(LIFT_PHASE),
        moved_to_the_host_just_now: false,
    };
    assert_eq!(
        call.tick(&facts),
        Some(GuestRideEvent::LostTrack {
            waited_ms: GUEST_LIFT_ARRIVAL_WATCH_MS
        })
    );
}

#[test]
fn the_host_refuses_a_call_from_another_map_or_for_a_non_lift_pick() {
    let mut ride = HostLiftRide::new();
    assert_eq!(
        ride.receive(a_call(), Some((LIFT_PHASE + 1, "A_MOUNT_ROBOT_02")), 0),
        Err(LiftCallOutcome::OtherMap)
    );
    assert_eq!(
        ride.receive(a_call(), None, 0),
        Err(LiftCallOutcome::OtherMap)
    );
    let boat = LiftCall {
        point: 701,
        origin: 700,
        ..a_call()
    };
    assert_eq!(
        ride.receive(boat, Some((LIFT_PHASE, "A_MOUNT_ROBOT_01")), 0),
        Err(LiftCallOutcome::NotALift)
    );
    assert!(!ride.in_flight());
}

#[test]
fn the_host_takes_one_call_at_a_time() {
    let mut ride = received();
    assert_eq!(
        ride.receive(a_call(), Some((LIFT_PHASE, "A_MOUNT_ROBOT_01")), 10),
        Err(LiftCallOutcome::AnotherRideRunning)
    );
}

#[test]
fn a_free_host_moves_to_the_panel_then_runs_the_guest_pick() {
    let mut ride = received();
    assert_eq!(ride.step(&free_host(0)), HostRideStep::Place(PANEL));
    assert_eq!(
        ride.step(&free_host(HOST_LIFT_PLACE_SETTLE_MS - 1)),
        HostRideStep::Settling
    );
    assert_eq!(
        ride.step(&free_host(HOST_LIFT_PLACE_SETTLE_MS)),
        HostRideStep::Run {
            phase: LIFT_PHASE,
            point: 401,
            origin: 400
        }
    );
}

#[test]
fn a_host_in_a_scene_talk_or_script_pause_waits_then_refuses() {
    for busy in [HostBusy::Scene, HostBusy::Talk, HostBusy::ScriptPause] {
        let mut ride = received();
        let facts = |now_ms| HostRideFacts {
            busy: Some(busy),
            ..free_host(now_ms)
        };
        assert_eq!(ride.step(&facts(1_000)), HostRideStep::Wait(busy));
        assert_eq!(
            ride.step(&facts(HOST_LIFT_BUSY_WAIT_MS)),
            HostRideStep::Refuse {
                phase: LIFT_PHASE,
                point: 401,
                outcome: LiftCallOutcome::HostBusy
            }
        );
        assert!(!ride.in_flight());
    }
}

#[test]
fn a_host_that_becomes_free_in_time_still_rides() {
    let mut ride = received();
    let busy = HostRideFacts {
        busy: Some(HostBusy::Talk),
        ..free_host(1_000)
    };
    assert_eq!(ride.step(&busy), HostRideStep::Wait(HostBusy::Talk));
    assert_eq!(ride.step(&free_host(2_000)), HostRideStep::Place(PANEL));
}

#[test]
fn the_host_pause_menu_is_closed_once_and_does_not_block_the_ride() {
    let mut ride = received();
    let menu = |now_ms| HostRideFacts {
        menu_open: true,
        ..free_host(now_ms)
    };
    assert_eq!(ride.step(&menu(0)), HostRideStep::CloseMenu);
    assert_eq!(ride.step(&menu(16)), HostRideStep::WaitForMenu);
    assert_eq!(ride.step(&free_host(32)), HostRideStep::Place(PANEL));
}

#[test]
fn the_host_drops_the_call_when_its_map_changes_before_the_run() {
    let mut ride = received();
    let elsewhere = HostRideFacts {
        phase: Some(LIFT_PHASE + 1),
        ..free_host(10)
    };
    assert_eq!(
        ride.step(&elsewhere),
        HostRideStep::Refuse {
            phase: LIFT_PHASE,
            point: 401,
            outcome: LiftCallOutcome::OtherMap
        }
    );
}

fn riding() -> HostLiftRide {
    let mut ride = received();
    ride.step(&free_host(0));
    ride.step(&free_host(HOST_LIFT_PLACE_SETTLE_MS));
    ride
}

#[test]
fn a_ride_inside_the_map_ends_when_the_script_pause_it_started_ends() {
    let mut ride = riding();
    let start = HOST_LIFT_PLACE_SETTLE_MS;
    assert_eq!(ride.step(&free_host(start + 100)), HostRideStep::Riding);
    let paused = HostRideFacts {
        script_pause: true,
        ..free_host(start + 200)
    };
    assert_eq!(ride.step(&paused), HostRideStep::Riding);
    assert_eq!(
        ride.step(&free_host(start + 9_000)),
        HostRideStep::Arrived {
            end: RideEnd::PauseEnded,
            waited_ms: 9_000
        }
    );
    assert!(!ride.in_flight());
}

#[test]
fn a_ride_that_changes_map_ends_in_the_new_map_and_a_loading_screen_is_not_the_end() {
    let mut ride = riding();
    let start = HOST_LIFT_PLACE_SETTLE_MS;
    let loading = HostRideFacts {
        phase: None,
        ..free_host(start + 500)
    };
    assert_eq!(ride.step(&loading), HostRideStep::Riding);
    let arrived = HostRideFacts {
        phase: Some(LIFT_PHASE + 1),
        ..free_host(start + 4_000)
    };
    assert_eq!(
        ride.step(&arrived),
        HostRideStep::Arrived {
            end: RideEnd::NewMap,
            waited_ms: 4_000
        }
    );
}

#[test]
fn the_host_stops_watching_a_ride_that_never_ends() {
    let mut ride = riding();
    let start = HOST_LIFT_PLACE_SETTLE_MS;
    let paused = HostRideFacts {
        script_pause: true,
        ..free_host(start + HOST_LIFT_RIDE_WATCH_MS)
    };
    assert_eq!(
        ride.step(&paused),
        HostRideStep::LostTrack {
            waited_ms: HOST_LIFT_RIDE_WATCH_MS
        }
    );
}
