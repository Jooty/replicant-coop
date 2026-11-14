use nc_launcher_core::{
    GuestSeatSync, HostStartGate, HostStore, LauncherEvent, SEAT_DELIVERY_TIMEOUT_SECONDS,
    SeatCandidate, SeatSource, StartGateStep, campaign_file_stem, host_store_decision,
    pick_newest_seat, plan_guest_start, session_file_name,
};
use replicant_game::story_progress::{SAVE_FILE_BYTES, session_file_with_seat};
use replicant_proto::SeatRefusal;

use crate::seat_assembler::generated_seat;

const GUEST_SLOT: u8 = 1;
const START_TRANSFER: u32 = 7;
const CAMPAIGN_ID: &str = "0123456789ABCDEF0123456789abcdef";

fn gate() -> HostStartGate {
    HostStartGate::new(GUEST_SLOT, START_TRANSFER, 100.0)
}

fn session_file(seat: &[u8]) -> Vec<u8> {
    session_file_with_seat(&vec![0; SAVE_FILE_BYTES], seat).expect("a session file")
}

fn stopped_with(step: StartGateStep, words: &str) -> bool {
    matches!(step, StartGateStep::Stop(ref text) if text.contains(words))
}

#[test]
fn the_host_starts_only_after_the_seat_arrived_whole() {
    let gate = gate();
    let waiting = [
        LauncherEvent::SeatProgress {
            slot: GUEST_SLOT,
            transfer: START_TRANSFER,
            parts_done: 40,
            parts: 79,
            incoming: false,
        },
        LauncherEvent::SeatDelivered {
            slot: GUEST_SLOT,
            transfer: START_TRANSFER + 1,
        },
        LauncherEvent::SeatDelivered {
            slot: 2,
            transfer: START_TRANSFER,
        },
        LauncherEvent::SeatReceived {
            slot: GUEST_SLOT,
            transfer: START_TRANSFER,
            seat: generated_seat(3),
        },
        LauncherEvent::GameSaved { succeeded: true },
    ];
    for event in &waiting {
        assert_eq!(gate.on_event(event), StartGateStep::Wait, "{event:?}");
    }
    assert_eq!(
        gate.on_tick(100.0 + SEAT_DELIVERY_TIMEOUT_SECONDS - 1.0),
        StartGateStep::Wait
    );
    assert_eq!(
        gate.on_event(&LauncherEvent::SeatDelivered {
            slot: GUEST_SLOT,
            transfer: START_TRANSFER,
        }),
        StartGateStep::Launch
    );
}

#[test]
fn a_refused_seat_stops_the_start_and_says_why() {
    let gate = gate();
    let refused = |reason, by_peer| LauncherEvent::SeatTransferRefused {
        slot: GUEST_SLOT,
        transfer: START_TRANSFER,
        reason,
        by_peer,
    };
    assert!(stopped_with(
        gate.on_event(&refused(SeatRefusal::BadChecksum, true)),
        "The guest's launcher refused the save (it was damaged on the way)"
    ));
    assert!(stopped_with(
        gate.on_event(&refused(SeatRefusal::NotASave, false)),
        "could not send the save (it is not a save the game can load)"
    ));
    assert!(stopped_with(
        gate.on_event(&LauncherEvent::MemberLeft {
            slot: GUEST_SLOT,
            detail: "timed out".to_owned(),
        }),
        "The guest left before their save arrived"
    ));
    assert!(stopped_with(
        gate.on_event(&LauncherEvent::Failed {
            message: "gone".to_owned()
        }),
        "The connection was lost"
    ));
    assert!(stopped_with(
        gate.on_tick(100.0 + SEAT_DELIVERY_TIMEOUT_SECONDS),
        "did not arrive within 60 s"
    ));
    assert_eq!(
        gate.on_event(&LauncherEvent::SeatTransferRefused {
            slot: GUEST_SLOT,
            transfer: START_TRANSFER - 1,
            reason: SeatRefusal::OutOfOrder,
            by_peer: true,
        }),
        StartGateStep::Wait
    );
}

#[test]
fn the_newer_seat_wins_by_its_own_play_time() {
    let candidate = |source, play_seconds| SeatCandidate {
        source,
        play_seconds,
    };
    assert_eq!(
        pick_newest_seat(&[
            candidate(SeatSource::Unsent, 90.0),
            candidate(SeatSource::FromHost, 120.0),
        ]),
        Some(SeatSource::FromHost)
    );
    assert_eq!(
        pick_newest_seat(&[
            candidate(SeatSource::Unsent, 120.0),
            candidate(SeatSource::FromHost, 120.0),
        ]),
        Some(SeatSource::Unsent)
    );
    assert_eq!(
        pick_newest_seat(&[
            candidate(SeatSource::SessionFile, f64::NAN),
            candidate(SeatSource::FromHost, 1.0),
        ]),
        Some(SeatSource::FromHost)
    );
    assert_eq!(pick_newest_seat(&[]), None);
    assert_eq!(host_store_decision(None, 10.0), HostStore::Store);
    assert_eq!(host_store_decision(Some(10.0), 10.5), HostStore::Store);
    assert_eq!(host_store_decision(Some(10.0), 10.0), HostStore::SetAside);
    assert_eq!(host_store_decision(Some(10.0), 9.0), HostStore::SetAside);
    assert_eq!(
        host_store_decision(Some(10.0), f64::INFINITY),
        HostStore::SetAside
    );
    let plan = plan_guest_start(300.0, None, Some(200.0));
    assert_eq!(
        (plan.boot_from, plan.send_back),
        (SeatSource::FromHost, false)
    );
    let plan = plan_guest_start(300.0, None, Some(301.0));
    assert_eq!(
        (plan.boot_from, plan.send_back),
        (SeatSource::SessionFile, true)
    );
}

#[test]
fn an_undelivered_seat_is_sent_again_next_session() {
    let started = generated_seat(3);
    let mut first = GuestSeatSync::new(
        CAMPAIGN_ID.to_owned(),
        session_file(&started),
        Some(started.clone()),
        None,
    );
    assert_eq!(first.on_saved(&session_file(&started)), Ok(None));
    let saved = generated_seat(5);
    let to_send = first.on_saved(&session_file(&saved)).expect("a valid save");
    assert_eq!(to_send.as_deref(), Some(saved.as_slice()));
    first.sending(11, saved.clone());
    assert_eq!(first.to_send(), None);
    assert_eq!(first.on_delivered(10), None);
    first.link_lost();
    assert_eq!(first.pending(), Some(saved.as_slice()));

    let plan = plan_guest_start(100.0, Some(150.0), Some(150.0));
    assert_eq!(plan.boot_from, SeatSource::Unsent);
    assert!(plan.send_back);
    let mut next = GuestSeatSync::new(
        CAMPAIGN_ID.to_owned(),
        session_file(&saved),
        Some(started),
        Some(saved.clone()),
    );
    assert_eq!(next.to_send().as_deref(), Some(saved.as_slice()));
    next.sending(1, saved.clone());
    assert_eq!(next.on_delivered(1).as_deref(), Some(saved.as_slice()));
    assert_eq!(next.pending(), None);
    assert_eq!(next.to_send(), None);
    assert_eq!(next.on_saved(&session_file(&saved)), Ok(None));
}

#[test]
fn a_save_into_another_slot_is_the_one_sent() {
    let started = generated_seat(3);
    let mut sync = GuestSeatSync::new(
        CAMPAIGN_ID.to_owned(),
        session_file(&started),
        Some(started.clone()),
        None,
    );
    let mut file = session_file(&started);
    let other = generated_seat(9);
    let at = replicant_game::story_progress::SLOTS_FILE_OFFSET
        + 2 * replicant_game::story_progress::SLOT_BYTES;
    file[at..at + other.len()].copy_from_slice(&other);
    assert_eq!(sync.on_saved(&file), Ok(Some(other)));
    let refused = generated_seat(4);
    sync.sending(3, refused);
    sync.on_refused(3);
    assert!(sync.to_send().is_some());
}

#[test]
fn a_session_file_is_named_for_its_campaign() {
    assert_eq!(
        session_file_name(CAMPAIGN_ID).as_deref(),
        Some("GAMEDATA.seat-01234567")
    );
    assert_eq!(campaign_file_stem(CAMPAIGN_ID), Some(CAMPAIGN_ID));
    for unsafe_id in [
        "",
        "0123456",
        "../../etc/passwd",
        "0123456789abcdeg",
        "ab cd ef 01",
    ] {
        assert_eq!(session_file_name(unsafe_id), None, "{unsafe_id}");
        assert_eq!(campaign_file_stem(unsafe_id), None, "{unsafe_id}");
    }
}
