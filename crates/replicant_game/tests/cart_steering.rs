use replicant_game::carts::{
    CART_BOARD_SETTLE_MS, CART_FRAME_SET_GAP_FRAMES, CART_HOST_RUN_SILENT_MS,
    CART_HOST_RUN_STILL_MS, CART_NEWER_MOTION_FLAG_OFFSET, CART_NEWER_MOTION_LAYER_COUNT_OFFSET,
    CART_NEWER_MOTION_LAYERS_OFFSET, CART_OLD_MOTION_LAYER_COUNT_OFFSET,
    CART_OLD_MOTION_LAYERS_OFFSET, CART_RUN_END_FRAME_OFFSET, CART_RUN_END_FRAME_WHEN_UNREAD,
    CART_STEERED_RATE_MAX, CART_STEERED_RATE_MIN, CartFollowStep, CartLeftAlone, CartMotion,
    CartSighting, CartState, CartSteer, GuestCartFollow, HostCartRun, HostCartRunTrack,
    MOTION_LAYER_SLOT_INDEX_OFFSET, MOTION_LAYER_SLOTS_OFFSET, NEWER_MOTION_SLOT_BYTES,
    OLD_MOTION_SLOT_BYTES, OwnCartFacts, read_cart_motion, read_cart_run_end_frame, steer_cart,
};
use replicant_proto::{CartRide, CartRun, CartRunProgress};

use crate::fake_memory::FakeMemory;

const END: f32 = CART_RUN_END_FRAME_WHEN_UNREAD;
const NOW: u64 = 50_000;
const CART: u64 = 0x5000_0000;
const LAYER: u64 = 0x5200_0000;
const SLOTS: u64 = 0x5300_0000;

fn host_at(frame: f32, rate: f32) -> HostCartRun {
    HostCartRun {
        seq: 7,
        frame,
        rate,
        received_ms: NOW,
        moved_ms: NOW,
    }
}

fn put_layer(memory: &mut FakeMemory, slot_bytes: u64, rate_offset: u64, motion_id: i32) {
    memory.put(LAYER, &0x5400_0000_u64.to_le_bytes());
    memory.put(LAYER + MOTION_LAYER_SLOT_INDEX_OFFSET, &2_i32.to_le_bytes());
    memory.put(LAYER + MOTION_LAYER_SLOTS_OFFSET, &SLOTS.to_le_bytes());
    let slot = SLOTS + 2 * slot_bytes;
    memory.put(slot, &motion_id.to_le_bytes());
    memory.put(slot + 4, &1_234.5_f32.to_le_bytes());
    memory.put(slot + rate_offset, &1.7_f32.to_le_bytes());
}

#[test]
fn a_small_gap_is_closed_by_rate() {
    let host = host_at(1_000.0, 1.0);
    let CartSteer::Rate { rate, gap } = steer_cart(Some(&host), 994.0, END, NOW) else {
        panic!("a six-frame gap is steered by rate");
    };
    assert_eq!(gap, 6.0);
    assert!(
        rate > 1.0,
        "behind the host: faster than the host, got {rate}"
    );
    let CartSteer::Rate { rate, gap } = steer_cart(Some(&host), 1_006.0, END, NOW) else {
        panic!("a six-frame lead is steered by rate");
    };
    assert_eq!(gap, -6.0);
    assert!(
        rate < 1.0,
        "ahead of the host: slower than the host, got {rate}"
    );
    let CartSteer::Rate { rate, .. } = steer_cart(Some(&host), 1_000.0, END, NOW) else {
        panic!("no gap is still steered");
    };
    assert_eq!(rate, 1.0, "no gap: the host's own rate");
}

#[test]
fn the_host_frame_is_carried_forward_from_its_arrival() {
    let host = host_at(1_000.0, 1.5);
    let CartSteer::Rate { gap, .. } = steer_cart(Some(&host), 1_009.0, END, NOW + 100) else {
        panic!("steered by rate");
    };
    assert!(
        gap.abs() < 0.01,
        "100 ms at rate 1.5 is 9 frames, got a gap of {gap}"
    );
}

#[test]
fn a_large_gap_sets_the_frame() {
    let host = host_at(2_000.0, 1.7);
    let behind = steer_cart(
        Some(&host),
        2_000.0 - CART_FRAME_SET_GAP_FRAMES - 5.0,
        END,
        NOW,
    );
    let CartSteer::SetFrame { frame, rate, .. } = behind else {
        panic!("far behind: the frame is set, got {behind:?}");
    };
    assert_eq!(frame, 2_000.0);
    assert_eq!(rate, 1.7);
    let ahead = steer_cart(Some(&host), 2_100.0, END, NOW);
    assert!(
        matches!(ahead, CartSteer::SetFrame { frame, .. } if frame == 2_000.0),
        "far ahead: set back to the host, got {ahead:?}"
    );
}

#[test]
fn the_frame_is_never_set_to_the_end_or_past_it() {
    for (host_frame, own_frame) in [(END + 200.0, END - 300.0), (END, 100.0), (END - 1.0, 0.0)] {
        let host = host_at(host_frame, 1.0);
        match steer_cart(Some(&host), own_frame, END, NOW) {
            CartSteer::SetFrame { frame, .. } => assert!(frame < END, "set to {frame}"),
            other => panic!("a gap this large sets the frame, got {other:?}"),
        }
    }
    let host = host_at(END + 200.0, 1.0);
    assert!(
        matches!(
            steer_cart(Some(&host), END - 4.0, END, NOW),
            CartSteer::Rate { rate, .. } if rate > 0.0
        ),
        "a cart already at the last settable frame runs out by rate"
    );
}

#[test]
fn the_rate_is_never_zero_or_negative() {
    for (host_rate, own_frame) in [
        (0.0, 1_020.0),
        (-3.0, 1_029.0),
        (0.3, 1_029.0),
        (5.0, 971.0),
    ] {
        let host = host_at(1_000.0, host_rate);
        let rate = match steer_cart(Some(&host), own_frame, END, NOW) {
            CartSteer::Rate { rate, .. } | CartSteer::SetFrame { rate, .. } => rate,
            CartSteer::LeaveAlone(why) => panic!("steered, not left alone ({why:?})"),
        };
        assert!(
            (CART_STEERED_RATE_MIN..=CART_STEERED_RATE_MAX).contains(&rate) && rate > 0.0,
            "host rate {host_rate}, own frame {own_frame}: rate {rate}"
        );
    }
}

#[test]
fn a_cart_without_a_host_ride_is_left_alone() {
    assert_eq!(
        steer_cart(None, 500.0, END, NOW),
        CartSteer::LeaveAlone(CartLeftAlone::NoHostRide)
    );
    let host = host_at(500.0, 1.0);
    assert_eq!(
        steer_cart(Some(&host), 500.0, END, NOW + CART_HOST_RUN_SILENT_MS + 1),
        CartSteer::LeaveAlone(CartLeftAlone::HostSilent)
    );
    assert_eq!(
        steer_cart(Some(&host), END + 0.5, END, NOW),
        CartSteer::LeaveAlone(CartLeftAlone::AtItsEnd)
    );
}

#[test]
fn a_paused_host_never_holds_the_guest_cart_back() {
    let mut track = HostCartRunTrack::new();
    assert!(track.receive(1, 800.0, 1.0, NOW));
    assert!(track.receive(2, 800.0, 1.0, NOW + 200));
    assert!(track.receive(3, 800.0, 1.0, NOW + CART_HOST_RUN_STILL_MS + 50));
    let host = track.latest().expect("a run");
    assert_eq!(
        steer_cart(Some(&host), 850.0, END, NOW + CART_HOST_RUN_STILL_MS + 60),
        CartSteer::LeaveAlone(CartLeftAlone::HostStill)
    );
    assert!(track.receive(4, 806.0, 1.0, NOW + CART_HOST_RUN_STILL_MS + 100));
    assert!(matches!(
        steer_cart(
            track.latest().as_ref(),
            850.0,
            END,
            NOW + CART_HOST_RUN_STILL_MS + 100
        ),
        CartSteer::SetFrame { .. }
    ));
}

#[test]
fn an_older_progress_message_never_replaces_a_newer_one() {
    let mut track = HostCartRunTrack::new();
    assert!(track.receive(5, 900.0, 1.0, NOW));
    assert!(!track.receive(4, 850.0, 1.0, NOW + 10));
    assert!(!track.receive(5, 850.0, 1.0, NOW + 10));
    assert_eq!(track.latest().map(|run| run.frame), Some(900.0));
    track.end(9);
    assert_eq!(track.latest(), None);
    assert!(
        !track.receive(8, 950.0, 1.0, NOW + 20),
        "nothing of the ended run"
    );
    assert!(
        track.receive(10, 20.0, 1.0, NOW + 30),
        "the next run is taken"
    );
    track.clear();
    assert_eq!(track.latest(), None);
}

#[test]
fn the_run_frame_is_read_from_either_motion_code() {
    let mut old = FakeMemory::new();
    old.put(CART + CART_NEWER_MOTION_FLAG_OFFSET, &[0]);
    old.put(
        CART + CART_OLD_MOTION_LAYER_COUNT_OFFSET,
        &3_i32.to_le_bytes(),
    );
    old.put(CART + CART_OLD_MOTION_LAYERS_OFFSET, &LAYER.to_le_bytes());
    put_layer(&mut old, OLD_MOTION_SLOT_BYTES, 0x14, 2);
    assert_eq!(
        read_cart_motion(&old, CART),
        Some(CartMotion {
            layer: LAYER,
            newer: false,
            frame: 1_234.5,
            rate: 1.7,
        })
    );
    let mut newer = FakeMemory::new();
    newer.put(CART + CART_NEWER_MOTION_FLAG_OFFSET, &[1]);
    newer.put(
        CART + CART_NEWER_MOTION_LAYER_COUNT_OFFSET,
        &1_i32.to_le_bytes(),
    );
    newer.put(CART + CART_NEWER_MOTION_LAYERS_OFFSET, &LAYER.to_le_bytes());
    put_layer(&mut newer, NEWER_MOTION_SLOT_BYTES, 0x10, 6);
    assert_eq!(
        read_cart_motion(&newer, CART),
        Some(CartMotion {
            layer: LAYER,
            newer: true,
            frame: 1_234.5,
            rate: 1.7,
        })
    );
    let mut empty = FakeMemory::new();
    empty.put(CART + CART_NEWER_MOTION_FLAG_OFFSET, &[0]);
    empty.put(
        CART + CART_OLD_MOTION_LAYER_COUNT_OFFSET,
        &3_i32.to_le_bytes(),
    );
    empty.put(CART + CART_OLD_MOTION_LAYERS_OFFSET, &LAYER.to_le_bytes());
    put_layer(&mut empty, OLD_MOTION_SLOT_BYTES, 0x14, -1);
    assert_eq!(read_cart_motion(&empty, CART), None, "no motion playing");
}

#[test]
fn the_end_frame_is_the_carts_own_or_the_known_one() {
    let mut memory = FakeMemory::new();
    assert_eq!(read_cart_run_end_frame(&memory, CART), END);
    memory.put(CART + CART_RUN_END_FRAME_OFFSET, &5_000_i32.to_le_bytes());
    assert_eq!(read_cart_run_end_frame(&memory, CART), 5_000.0);
}

#[test]
fn only_the_run_the_guest_rides_is_steered() {
    let mut follow = GuestCartFollow::new();
    let run = CartRun {
        phase: 13,
        layout_id: Some(41),
        cart_position: [10.0, 0.0, -105.0],
        run_state: 5,
        seq: 1,
        progress: CartRunProgress::Moving {
            frame: 20.0,
            rate: 1.0,
        },
    };
    assert_eq!(follow.steered_cart(&run), None, "not riding");
    let ride = CartRide {
        phase: 13,
        layout_id: Some(41),
        cart_position: [10.0, 0.0, -105.0],
        from_state: 1,
        to_state: 5,
        boarded_at: [12.0, 0.0, -115.0],
    };
    let own = [CartSighting {
        actor: CART,
        layout_id: Some(41),
        state: CartState::Parked(1),
        position: [10.0, 0.0, -105.0],
    }];
    let facts = |now_ms, rider_on_cart| OwnCartFacts {
        now_ms,
        phase: Some(13),
        carts: &own,
        may_place_rider: true,
        rider_on_cart,
    };
    follow.host_rode(ride, 0);
    follow.step(&facts(0, false));
    follow.step(&facts(CART_BOARD_SETTLE_MS, false));
    assert_eq!(
        follow.step(&facts(CART_BOARD_SETTLE_MS + 16, true)),
        CartFollowStep::Boarded { actor: CART }
    );
    assert_eq!(follow.steered_cart(&run), Some(CART));
    assert_eq!(follow.steered_actor(), Some(CART));
    for other in [
        CartRun {
            run_state: 7,
            ..run
        },
        CartRun { phase: 14, ..run },
        CartRun {
            layout_id: Some(42),
            ..run
        },
    ] {
        assert_eq!(follow.steered_cart(&other), None, "{other:?}");
    }
    let ended = CartRide {
        from_state: 5,
        to_state: 2,
        ..ride
    };
    follow.host_rode(ended, 30_000);
    assert_eq!(
        follow.steered_cart(&run),
        None,
        "the host's ride ended: no longer steered"
    );
    assert_eq!(follow.steered_actor(), None);
}
