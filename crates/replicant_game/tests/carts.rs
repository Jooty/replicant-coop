use replicant_game::Position;
use replicant_game::carts::{
    CART_BOARD_CONFIRM_MS, CART_BOARD_GIVE_UP_MS, CART_BOARD_SETTLE_MS, CART_BOARD_TRIES,
    CART_END_GRACE_MS, CART_END_PULL_DELAY_MS, CART_MATCH_UNITS, CART_PARTNER_NEAR_UNITS,
    CART_RIDER_LIFT_UNITS, CART_STATE_OFFSET, CART_VTABLE_RVA, CartFollowStep, CartRideEnd,
    CartRideEvent, CartRideFacts, CartSighting, CartState, GuestCartFollow, HostRideWatch,
    OwnCartFacts, cart_follow_step_text, cart_ride_event_text, cart_state, match_cart,
    match_ride_cart, parked_state_after, run_state_from, scan_cart_actors,
};
use replicant_game::offsets::{
    ACTOR_LIFE_STATE_OFFSET, ACTOR_MANAGER_RVA, ACTOR_MANAGER_SECONDARY_LIST_HEAD_OFFSET,
    ACTOR_SECONDARY_LIST_NEXT_OFFSET, ACTOR_SELF_HANDLE_OFFSET, ACTOR_TRANSFORM_BLOCK_BYTES,
    ACTOR_TRANSFORM_BLOCK_OFFSET, HANDLE_POOL_GENERATIONS_RVA, HANDLE_POOL_POINTERS_RVA,
};
use replicant_game::puppet::PuppetPose;
use replicant_game::stay_together::{JumpPartner, JumpStep};
use replicant_proto::CartRide;

use crate::fake_memory::{FAKE_MODULE_BASE, FakeMemory};

const PHASE: u16 = 13;
const CART: u64 = 0x5000_0000;
const OTHER_CART: u64 = 0x5100_0000;
const GUEST_CART: u64 = 0x6000_0000;
const PARKED_AT: [f32; 3] = [10.0, 0.0, -105.0];
const BOARDED_AT: [f32; 3] = [12.0, 0.0, -115.0];
const RAIL_END: [f32; 3] = [-37.0, -0.5, -42.0];

fn cart(actor: u64, state: CartState, position: [f32; 3]) -> CartSighting {
    CartSighting {
        actor,
        layout_id: Some(41),
        state,
        position,
    }
}

fn plus(position: [f32; 3], offset: [f32; 3]) -> [f32; 3] {
    [
        position[0] + offset[0],
        position[1] + offset[1],
        position[2] + offset[2],
    ]
}

fn host_facts(now_ms: u64, carts: &[CartSighting], rider_on_cart: bool) -> CartRideFacts<'_> {
    host_at(now_ms, carts, rider_on_cart, BOARDED_AT)
}

fn host_at(
    now_ms: u64,
    carts: &[CartSighting],
    rider_on_cart: bool,
    at: [f32; 3],
) -> CartRideFacts<'_> {
    CartRideFacts {
        now_ms,
        phase: Some(PHASE),
        carts,
        rider_on_cart,
        rider_position: Some(at),
    }
}

fn started_ride() -> CartRide {
    CartRide {
        phase: PHASE,
        layout_id: Some(41),
        cart_position: PARKED_AT,
        from_state: 1,
        to_state: 5,
        boarded_at: BOARDED_AT,
    }
}

fn ended_ride() -> CartRide {
    CartRide {
        phase: PHASE,
        layout_id: Some(41),
        cart_position: PARKED_AT,
        from_state: 5,
        to_state: 2,
        boarded_at: RAIL_END,
    }
}

fn own_facts(now_ms: u64, carts: &[CartSighting], may_place_rider: bool) -> OwnCartFacts<'_> {
    OwnCartFacts {
        now_ms,
        phase: Some(PHASE),
        carts,
        may_place_rider,
        rider_on_cart: false,
    }
}

fn aboard(facts: OwnCartFacts<'_>) -> OwnCartFacts<'_> {
    OwnCartFacts {
        rider_on_cart: true,
        ..facts
    }
}

fn host_rides_once(watch: &mut HostRideWatch) {
    let parked = [cart(CART, CartState::Parked(1), PARKED_AT)];
    watch.observe(&host_at(0, &parked, false, BOARDED_AT));
    watch.observe(&host_at(100, &parked, true, BOARDED_AT));
    let running = [cart(CART, CartState::Running(5), PARKED_AT)];
    watch.observe(&host_at(
        200,
        &running,
        true,
        plus(BOARDED_AT, [0.0, 0.0, 3.0]),
    ));
}

fn guest_boards(follow: &mut GuestCartFollow, own: &[CartSighting]) {
    follow.host_rode(started_ride(), 0);
    assert!(matches!(
        follow.step(&own_facts(0, own, true)),
        CartFollowStep::PlaceRider { .. }
    ));
    assert!(matches!(
        follow.step(&own_facts(CART_BOARD_SETTLE_MS, own, true)),
        CartFollowStep::Board { .. }
    ));
    assert_eq!(
        follow.step(&aboard(own_facts(CART_BOARD_SETTLE_MS + 16, own, true))),
        CartFollowStep::Boarded { actor: GUEST_CART }
    );
}

fn put_cart(memory: &mut FakeMemory, actor: u64, slot: u32, next: u64, state: i32) {
    memory.put(
        actor,
        &(FAKE_MODULE_BASE + u64::from(CART_VTABLE_RVA)).to_le_bytes(),
    );
    memory.put(
        actor + ACTOR_SECONDARY_LIST_NEXT_OFFSET,
        &next.to_le_bytes(),
    );
    let mut handle = [0_u8; 8];
    handle[..4].copy_from_slice(&slot.to_le_bytes());
    handle[4..].copy_from_slice(&1_u32.to_le_bytes());
    memory.put(actor + ACTOR_SELF_HANDLE_OFFSET, &handle);
    memory.put_at_rva(HANDLE_POOL_POINTERS_RVA + slot * 8, &actor.to_le_bytes());
    memory.put_at_rva(HANDLE_POOL_GENERATIONS_RVA + slot * 4, &1_u32.to_le_bytes());
    memory.put(actor + ACTOR_LIFE_STATE_OFFSET, &0_i32.to_le_bytes());
    memory.put(actor + CART_STATE_OFFSET, &state.to_le_bytes());
    let pose = PuppetPose {
        position: Position {
            x: 10.0,
            y: 0.0,
            z: -105.0,
        },
        yaw: 0.0,
    };
    memory.put(
        actor + ACTOR_TRANSFORM_BLOCK_OFFSET,
        &pose.creation_matrix()[..ACTOR_TRANSFORM_BLOCK_BYTES],
    );
}

#[test]
fn boarding_runs_the_parked_end_and_a_run_parks_at_the_other_end() {
    assert_eq!(run_state_from(1), Some(5));
    assert_eq!(run_state_from(4), Some(8));
    assert_eq!(run_state_from(5), None);
    assert_eq!(parked_state_after(5), Some(2));
    assert_eq!(parked_state_after(6), Some(1));
    assert_eq!(parked_state_after(7), Some(4));
    assert_eq!(parked_state_after(8), Some(3));
    assert_eq!(parked_state_after(2), None);
    assert_eq!(cart_state(3), CartState::Parked(3));
    assert_eq!(cart_state(7), CartState::Running(7));
    assert_eq!(cart_state(0), CartState::Unset);
    assert_eq!(cart_state(200), CartState::Unset);
}

#[test]
fn the_scan_lists_live_carts_on_the_actor_list_with_their_state_and_place() {
    let mut memory = FakeMemory::new();
    let head =
        u32::try_from(u64::from(ACTOR_MANAGER_RVA) + ACTOR_MANAGER_SECONDARY_LIST_HEAD_OFFSET)
            .expect("rva");
    memory.put_at_rva(head, &CART.to_le_bytes());
    put_cart(&mut memory, CART, 1, OTHER_CART, 3);
    put_cart(&mut memory, OTHER_CART, 2, 0, 6);
    memory.put(OTHER_CART, &0x1234_u64.to_le_bytes());
    let found = scan_cart_actors(&memory, |actor| (actor == CART).then_some(7));
    assert_eq!(
        found,
        vec![CartSighting {
            actor: CART,
            layout_id: Some(7),
            state: CartState::Parked(3),
            position: PARKED_AT,
        }]
    );
}

#[test]
fn a_report_finds_the_cart_by_layout_id_first_then_by_place() {
    let near = cart(CART, CartState::Parked(1), [12.0, 0.0, -105.0]);
    let far = CartSighting {
        layout_id: Some(9),
        ..cart(OTHER_CART, CartState::Parked(2), [90.0, 0.0, 0.0])
    };
    assert_eq!(
        match_cart(&[near, far], Some(9), PARKED_AT).map(|c| c.actor),
        Some(OTHER_CART)
    );
    assert_eq!(
        match_cart(&[near, far], Some(77), PARKED_AT).map(|c| c.actor),
        Some(CART)
    );
    let too_far = cart(
        CART,
        CartState::Parked(1),
        plus(PARKED_AT, [CART_MATCH_UNITS + 1.0, 0.0, 0.0]),
    );
    assert_eq!(match_cart(&[too_far], None, PARKED_AT), None);
}

#[test]
fn the_host_reports_where_it_boarded_however_far_the_cart_actor_is() {
    let mut watch = HostRideWatch::new();
    let parked = [cart(CART, CartState::Parked(1), PARKED_AT)];
    assert!(
        watch
            .observe(&host_at(0, &parked, false, BOARDED_AT))
            .is_empty()
    );
    assert!(
        watch
            .observe(&host_at(100, &parked, true, BOARDED_AT))
            .is_empty()
    );
    let moving = [cart(CART, CartState::Running(5), PARKED_AT)];
    let later = plus(BOARDED_AT, [0.0, 0.0, 3.0]);
    let events = watch.observe(&host_at(200, &moving, true, later));
    let [CartRideEvent::Started(ride)] = events.as_slice() else {
        panic!("one start, got {events:?}");
    };
    assert_eq!(ride.cart_position, PARKED_AT);
    assert_eq!((ride.from_state, ride.to_state), (1, 5));
    assert_eq!(ride.boarded_at, BOARDED_AT);
    assert!(ride.validate().is_ok());
    assert!(watch.riding());
    assert!(cart_ride_event_text(&events[0], true).contains("host boarded at (12.0, 0.0, -115.0)"));
}

#[test]
fn a_start_gated_ride_still_reports_the_spot_the_host_boarded_from() {
    let mut watch = HostRideWatch::new();
    let parked = [cart(CART, CartState::Parked(1), PARKED_AT)];
    watch.observe(&host_at(0, &parked, false, BOARDED_AT));
    watch.observe(&host_at(100, &parked, true, BOARDED_AT));
    watch.observe(&host_at(5_000, &parked, true, BOARDED_AT));
    let moving = [cart(CART, CartState::Running(5), PARKED_AT)];
    let events = watch.observe(&host_at(5_100, &moving, true, [0.0, 0.0, -90.0]));
    let [CartRideEvent::Started(ride)] = events.as_slice() else {
        panic!("one start, got {events:?}");
    };
    assert_eq!(ride.boarded_at, BOARDED_AT);
}

#[test]
fn a_cart_that_runs_without_the_host_on_it_is_not_a_ride() {
    let mut watch = HostRideWatch::new();
    let parked = [cart(CART, CartState::Parked(1), PARKED_AT)];
    watch.observe(&host_facts(0, &parked, false));
    let moving = [cart(CART, CartState::Running(5), PARKED_AT)];
    assert!(watch.observe(&host_facts(100, &moving, false)).is_empty());
    let back = [cart(CART, CartState::Parked(2), PARKED_AT)];
    assert!(watch.observe(&host_facts(200, &back, false)).is_empty());
}

#[test]
fn the_ride_ends_when_the_game_clears_the_on_cart_flag_and_a_far_partner_is_brought() {
    let mut watch = HostRideWatch::new();
    host_rides_once(&mut watch);
    let parked = [cart(CART, CartState::Parked(2), PARKED_AT)];
    let events = watch.observe(&host_at(30_000, &parked, false, RAIL_END));
    let [
        CartRideEvent::Ended {
            end: CartRideEnd::OnCartFlagCleared,
            parked: Some(ride),
            at,
        },
    ] = events.as_slice()
    else {
        panic!("one end by the flag, got {events:?}");
    };
    assert_eq!((ride.from_state, ride.to_state), (5, 2));
    assert_eq!(*at, RAIL_END);
    let far = Some(JumpPartner {
        position: BOARDED_AT,
        phase: PHASE,
    });
    assert_eq!(
        watch.pull_due(30_000, Some(RAIL_END), far),
        JumpStep::Nothing
    );
    let due = 30_000 + CART_END_PULL_DELAY_MS;
    let JumpStep::BringPartner { from, to, .. } = watch.pull_due(due, Some(RAIL_END), far) else {
        panic!("the far partner is brought");
    };
    assert_eq!((from, to), (BOARDED_AT, RAIL_END));
    assert_eq!(
        watch.pull_due(due + 1, Some(RAIL_END), far),
        JumpStep::Nothing
    );
}

#[test]
fn the_on_cart_flag_clearing_mid_rail_ends_the_ride_with_nothing_sent_but_the_pull() {
    let mut watch = HostRideWatch::new();
    host_rides_once(&mut watch);
    let running = [cart(CART, CartState::Running(5), PARKED_AT)];
    let events = watch.observe(&host_at(9_000, &running, false, BOARDED_AT));
    assert!(matches!(
        events.as_slice(),
        [CartRideEvent::Ended {
            end: CartRideEnd::OnCartFlagCleared,
            parked: None,
            ..
        }]
    ));
    assert!(!watch.riding());
    let far = Some(JumpPartner {
        position: RAIL_END,
        phase: PHASE,
    });
    let due = 9_000 + CART_END_PULL_DELAY_MS;
    assert!(matches!(
        watch.pull_due(due, Some(BOARDED_AT), far),
        JumpStep::BringPartner { .. }
    ));
}

#[test]
fn a_cart_removed_under_the_host_ends_the_ride() {
    let mut watch = HostRideWatch::new();
    host_rides_once(&mut watch);
    let new_cart = [cart(OTHER_CART, CartState::Parked(1), PARKED_AT)];
    let events = watch.observe(&host_at(9_000, &new_cart, true, BOARDED_AT));
    assert!(matches!(
        events.as_slice(),
        [CartRideEvent::Ended {
            end: CartRideEnd::CartGone,
            parked: None,
            ..
        }]
    ));
    assert!(cart_ride_event_text(&events[0], true).contains("cart actor removed"));
    assert!(!watch.riding());
}

#[test]
fn a_partner_who_rode_along_or_is_in_another_map_is_left_where_it_is() {
    let ride_once = |watch: &mut HostRideWatch| {
        host_rides_once(watch);
        watch.observe(&host_at(
            300,
            &[cart(CART, CartState::Parked(2), PARKED_AT)],
            false,
            RAIL_END,
        ));
    };
    let due = 300 + CART_END_PULL_DELAY_MS;
    let mut watch = HostRideWatch::new();
    ride_once(&mut watch);
    let beside = Some(JumpPartner {
        position: plus(RAIL_END, [CART_PARTNER_NEAR_UNITS - 1.0, 0.0, 0.0]),
        phase: PHASE,
    });
    assert_eq!(
        watch.pull_due(due, Some(RAIL_END), beside),
        JumpStep::Nothing
    );
    let mut watch = HostRideWatch::new();
    ride_once(&mut watch);
    let elsewhere = Some(JumpPartner {
        position: PARKED_AT,
        phase: PHASE + 1,
    });
    assert_eq!(
        watch.pull_due(due, Some(RAIL_END), elsewhere),
        JumpStep::Nothing
    );
}

#[test]
fn a_map_change_forgets_what_the_host_saw() {
    let mut watch = HostRideWatch::new();
    watch.observe(&host_facts(
        0,
        &[cart(CART, CartState::Parked(1), PARKED_AT)],
        false,
    ));
    let mut other_map = host_facts(100, &[], false);
    other_map.phase = Some(PHASE + 1);
    watch.observe(&other_map);
    let running = [cart(CART, CartState::Running(5), PARKED_AT)];
    assert!(watch.observe(&host_facts(200, &running, true)).is_empty());
}

#[test]
fn a_ride_cart_without_a_layout_id_is_matched_by_place_and_parked_state() {
    let ride = CartRide {
        layout_id: None,
        ..started_ride()
    };
    let nearest_wrong_end = CartSighting {
        layout_id: None,
        ..cart(OTHER_CART, CartState::Parked(2), PARKED_AT)
    };
    let at_the_start_end = CartSighting {
        layout_id: None,
        ..cart(
            GUEST_CART,
            CartState::Parked(1),
            plus(PARKED_AT, [0.6, 0.0, 0.0]),
        )
    };
    assert_eq!(
        match_ride_cart(&[nearest_wrong_end, at_the_start_end], &ride).map(|c| c.actor),
        Some(GUEST_CART)
    );
    assert_eq!(
        match_ride_cart(&[nearest_wrong_end], &ride).map(|c| c.actor),
        Some(OTHER_CART)
    );
    let too_far = CartSighting {
        layout_id: None,
        ..cart(
            GUEST_CART,
            CartState::Parked(1),
            plus(PARKED_AT, [CART_MATCH_UNITS + 1.0, 0.0, 0.0]),
        )
    };
    assert_eq!(match_ride_cart(&[too_far], &ride), None);
}

#[test]
fn the_guest_stands_where_the_host_boarded_then_the_game_boards_it() {
    let mut follow = GuestCartFollow::new();
    follow.host_rode(started_ride(), 0);
    let own = [cart(GUEST_CART, CartState::Parked(1), PARKED_AT)];
    let CartFollowStep::PlaceRider { actor, spot } = follow.step(&own_facts(0, &own, true)) else {
        panic!("the rider is placed first");
    };
    assert_eq!(actor, GUEST_CART);
    assert_eq!(spot, plus(BOARDED_AT, [0.0, CART_RIDER_LIFT_UNITS, 0.0]));
    assert_eq!(
        follow.step(&own_facts(CART_BOARD_SETTLE_MS - 1, &own, true)),
        CartFollowStep::Nothing
    );
    let board = follow.step(&own_facts(CART_BOARD_SETTLE_MS, &own, true));
    assert_eq!(
        board,
        CartFollowStep::Board {
            actor: GUEST_CART,
            try_number: 1
        }
    );
    assert!(cart_follow_step_text(&board).is_some_and(|text| text.contains("own boarding")));
    let boarded = follow.step(&aboard(own_facts(CART_BOARD_SETTLE_MS + 16, &own, true)));
    assert_eq!(boarded, CartFollowStep::Boarded { actor: GUEST_CART });
    assert!(cart_follow_step_text(&boarded).is_some_and(|text| text.contains("on-cart flag 1")));
    assert_eq!(follow.riding_actor(), Some(GUEST_CART));
}

#[test]
fn the_mod_never_runs_the_guest_cart_itself() {
    let mut follow = GuestCartFollow::new();
    follow.host_rode(started_ride(), 0);
    let own = [cart(GUEST_CART, CartState::Parked(1), PARKED_AT)];
    for now in (0..CART_BOARD_GIVE_UP_MS).step_by(50) {
        let step = follow.step(&own_facts(now, &own, true));
        if let CartFollowStep::Park { state, .. } | CartFollowStep::ParkAtEnd { state, .. } = step {
            assert!(matches!(cart_state(i32::from(state)), CartState::Parked(_)));
        }
    }
}

#[test]
fn a_refused_boarding_is_asked_again_then_left_to_the_end_pull() {
    let mut follow = GuestCartFollow::new();
    follow.host_rode(started_ride(), 0);
    let own = [cart(GUEST_CART, CartState::Parked(1), PARKED_AT)];
    let mut boards = 0;
    let mut now = 0;
    let last = loop {
        let step = follow.step(&own_facts(now, &own, true));
        if matches!(step, CartFollowStep::Board { .. }) {
            boards += 1;
        }
        if matches!(
            step,
            CartFollowStep::BoardRefused { .. } | CartFollowStep::GaveUpBoarding { .. }
        ) {
            break step;
        }
        now += 50;
        assert!(now <= CART_BOARD_GIVE_UP_MS, "gave up in time");
    };
    assert_eq!(boards, CART_BOARD_TRIES);
    assert_eq!(
        last,
        CartFollowStep::BoardRefused {
            tries: CART_BOARD_TRIES
        }
    );
    assert!(
        now >= u64::from(CART_BOARD_TRIES) * (CART_BOARD_SETTLE_MS + CART_BOARD_CONFIRM_MS) - 100
    );
    assert_eq!(
        follow.step(&own_facts(now + 50, &own, true)),
        CartFollowStep::Nothing
    );
}

#[test]
fn a_guest_cart_at_the_other_end_is_brought_to_the_host_start_first() {
    let mut follow = GuestCartFollow::new();
    follow.host_rode(started_ride(), 0);
    let wrong_end = [cart(GUEST_CART, CartState::Parked(2), PARKED_AT)];
    assert_eq!(
        follow.step(&own_facts(0, &wrong_end, true)),
        CartFollowStep::Park {
            actor: GUEST_CART,
            state: 1
        }
    );
    let parked = [cart(GUEST_CART, CartState::Parked(1), PARKED_AT)];
    assert!(matches!(
        follow.step(&own_facts(100, &parked, true)),
        CartFollowStep::PlaceRider { .. }
    ));
}

#[test]
fn a_guest_in_a_talk_waits_and_then_gives_the_ride_up_to_the_end_pull() {
    let mut follow = GuestCartFollow::new();
    follow.host_rode(started_ride(), 0);
    let own = [cart(GUEST_CART, CartState::Parked(1), PARKED_AT)];
    assert_eq!(
        follow.step(&own_facts(100, &own, false)),
        CartFollowStep::Nothing
    );
    let step = follow.step(&own_facts(CART_BOARD_GIVE_UP_MS, &own, false));
    assert!(matches!(step, CartFollowStep::GaveUpBoarding { .. }));
    assert!(cart_follow_step_text(&step).is_some());
    assert_eq!(
        follow.step(&own_facts(CART_BOARD_GIVE_UP_MS + 100, &own, true)),
        CartFollowStep::Nothing
    );
}

#[test]
fn a_guest_without_a_matching_cart_or_in_another_map_is_left_to_the_end_pull() {
    let mut follow = GuestCartFollow::new();
    follow.host_rode(started_ride(), 0);
    let step = follow.step(&own_facts(0, &[], true));
    assert!(matches!(step, CartFollowStep::NoCartHere { .. }));
    let mut follow = GuestCartFollow::new();
    follow.host_rode(started_ride(), 0);
    let own = [cart(GUEST_CART, CartState::Parked(1), PARKED_AT)];
    let mut elsewhere = own_facts(0, &own, true);
    elsewhere.phase = Some(PHASE + 1);
    assert_eq!(follow.step(&elsewhere), CartFollowStep::OtherMap);
}

#[test]
fn the_guests_own_ride_ends_by_its_own_flag_or_its_cart_going() {
    let own = [cart(GUEST_CART, CartState::Parked(1), PARKED_AT)];
    let running = [cart(GUEST_CART, CartState::Running(5), PARKED_AT)];
    let mut follow = GuestCartFollow::new();
    guest_boards(&mut follow, &own);
    assert_eq!(
        follow.step(&aboard(own_facts(5_000, &running, true))),
        CartFollowStep::Nothing
    );
    let parked_far = [cart(GUEST_CART, CartState::Parked(2), PARKED_AT)];
    let step = follow.step(&own_facts(20_000, &parked_far, true));
    assert_eq!(
        step,
        CartFollowStep::OwnRideEnded {
            actor: GUEST_CART,
            end: CartRideEnd::OnCartFlagCleared
        }
    );
    assert!(cart_follow_step_text(&step).is_some_and(|text| text.contains("1 -> 0")));
    let mut follow = GuestCartFollow::new();
    guest_boards(&mut follow, &own);
    assert_eq!(
        follow.step(&aboard(own_facts(5_000, &[], true))),
        CartFollowStep::OwnRideEnded {
            actor: GUEST_CART,
            end: CartRideEnd::CartGone
        }
    );
}

#[test]
fn the_host_ending_first_never_parks_a_cart_the_guest_still_rides() {
    let own = [cart(GUEST_CART, CartState::Parked(1), PARKED_AT)];
    let running = [cart(GUEST_CART, CartState::Running(5), PARKED_AT)];
    let mut follow = GuestCartFollow::new();
    guest_boards(&mut follow, &own);
    follow.host_rode(ended_ride(), 20_000);
    assert_eq!(
        follow.step(&aboard(own_facts(
            20_000 + CART_END_GRACE_MS,
            &running,
            true
        ))),
        CartFollowStep::Nothing
    );
    let parked_far = [cart(GUEST_CART, CartState::Parked(2), PARKED_AT)];
    assert!(matches!(
        follow.step(&own_facts(21_000 + CART_END_GRACE_MS, &parked_far, true)),
        CartFollowStep::OwnRideEnded { .. }
    ));
    assert_eq!(
        follow.step(&own_facts(21_100 + CART_END_GRACE_MS, &parked_far, true)),
        CartFollowStep::Nothing
    );
}

#[test]
fn a_guest_cart_left_at_the_start_is_put_at_the_host_end() {
    let mut follow = GuestCartFollow::new();
    follow.host_rode(ended_ride(), 20_000);
    let start = [cart(GUEST_CART, CartState::Parked(1), PARKED_AT)];
    assert_eq!(
        follow.step(&own_facts(20_000 + CART_END_GRACE_MS - 1, &start, true)),
        CartFollowStep::Nothing
    );
    assert_eq!(
        follow.step(&own_facts(20_000 + CART_END_GRACE_MS, &start, true)),
        CartFollowStep::ParkAtEnd {
            actor: GUEST_CART,
            state: 2
        }
    );
}

#[test]
fn a_guest_cart_that_reached_the_same_end_is_left_alone() {
    let mut follow = GuestCartFollow::new();
    follow.host_rode(ended_ride(), 0);
    let arrived = [cart(GUEST_CART, CartState::Parked(2), PARKED_AT)];
    assert_eq!(
        follow.step(&own_facts(CART_END_GRACE_MS, &arrived, true)),
        CartFollowStep::Nothing
    );
}

#[test]
fn a_ride_ends_when_the_flag_clears_the_state_leaves_or_the_cart_goes() {
    let ends = |carts: &[CartSighting], on_cart: bool, phase: Option<u16>| {
        let mut watch = HostRideWatch::new();
        host_rides_once(&mut watch);
        assert!(
            watch
                .ridden_cart()
                .is_some_and(|ridden| ridden.actor == CART
                    && ridden.run_state == 5
                    && ridden.start_position == PARKED_AT)
        );
        let mut facts = host_at(9_000, carts, on_cart, BOARDED_AT);
        facts.phase = phase;
        let events = watch.observe(&facts);
        assert!(!watch.riding(), "the ride is over after {events:?}");
        assert_eq!(watch.ridden_cart(), None);
        match events.as_slice() {
            [CartRideEvent::Ended { end, .. }] => *end,
            other => panic!("one end, got {other:?}"),
        }
    };
    let running = [cart(CART, CartState::Running(5), PARKED_AT)];
    let parked_far = [cart(CART, CartState::Parked(2), PARKED_AT)];
    let parked_back = [cart(CART, CartState::Parked(1), PARKED_AT)];
    let unset = [cart(CART, CartState::Unset, PARKED_AT)];
    assert_eq!(
        ends(&running, false, Some(PHASE)),
        CartRideEnd::OnCartFlagCleared
    );
    assert_eq!(
        ends(&parked_far, true, Some(PHASE)),
        CartRideEnd::CartParked
    );
    assert_eq!(
        ends(&parked_back, true, Some(PHASE)),
        CartRideEnd::StateLeftRun
    );
    assert_eq!(ends(&unset, true, Some(PHASE)), CartRideEnd::StateLeftRun);
    assert_eq!(ends(&[], true, Some(PHASE)), CartRideEnd::CartGone);
    assert_eq!(
        ends(&running, true, Some(PHASE + 3)),
        CartRideEnd::PhaseChanged
    );
    assert_eq!(ends(&running, true, None), CartRideEnd::PhaseChanged);

    let mut still_riding = HostRideWatch::new();
    host_rides_once(&mut still_riding);
    assert!(
        still_riding
            .observe(&host_at(9_000, &running, true, BOARDED_AT))
            .is_empty()
    );
    assert!(still_riding.riding());
}
