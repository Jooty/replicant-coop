use std::sync::Arc;

use nc_launcher_core::{
    HOST_KEY_NOT_FOUND_TEXT, LauncherConfig, LauncherCore, LauncherEvent, LauncherNet, NetState,
    PathKind, iroh_factory,
};
use nc_net::{IrohConfig, LinkProgress, LookupMeans};

use crate::bridge::{FakeGame, state_at, wait_until};

const HOST_AND_JOIN_LOOKUP_PORT: u16 = 47_571;
const CONNECTION_TEST_LOOKUP_PORT: u16 = 47_572;
const PROGRESS_LOOKUP_PORT: u16 = 47_573;
const ANSWERED_ELSEWHERE_LOOKUP_PORT: u16 = 47_574;
const UNANSWERED_LOOKUP_PORT: u16 = 47_575;

fn local_core(name: &str, local_lookup_port: u16) -> LauncherCore {
    let factory = iroh_factory(IrohConfig {
        use_relays_and_lookup: false,
        local_lookup_port,
    });
    LauncherCore::new(LauncherConfig::new(name), Arc::clone(&factory)).expect("core")
}

#[test]
fn two_launchers_connect_over_local_iroh_and_relay_state() {
    let host = local_core("Hosty", HOST_AND_JOIN_LOOKUP_PORT);
    let guest = local_core("Guesty", HOST_AND_JOIN_LOOKUP_PORT);
    let mut host_game = FakeGame::attach(&host, &LauncherConfig::new("x"));
    let mut guest_game = FakeGame::attach(&guest, &LauncherConfig::new("x"));
    let ticket = host.host().expect("ticket");
    guest.join(&ticket).expect("join");
    assert!(wait_until(|| matches!(
        guest.status().state,
        NetState::Connected { .. }
    )));
    let mut tick = 0;
    assert!(wait_until(|| {
        tick += 1;
        host_game.send_state(&state_at(tick, 9.0));
        guest_game.saw(|m| matches!(m, replicant_proto::Message::State(s) if s.position[0] == 9.0))
    }));
}

#[test]
fn a_connection_test_over_local_iroh_finds_a_direct_path() {
    let host = local_core("Hosty", CONNECTION_TEST_LOOKUP_PORT);
    let guest = local_core("Guesty", CONNECTION_TEST_LOOKUP_PORT);
    let ticket = host.host().expect("ticket");
    let report = guest.test_connection(&ticket, 4).expect("probe runs");
    assert!(report.reachable);
    assert!(report.rtt.is_some());
    assert_eq!(report.path, PathKind::Direct);
}

#[test]
fn a_join_over_local_iroh_tells_its_progress_and_logs_it() {
    let host = local_core("Hosty", PROGRESS_LOOKUP_PORT);
    let guest = local_core("Guesty", PROGRESS_LOOKUP_PORT);
    let ticket = host.host().expect("ticket");
    assert!(
        host.progress()
            .iter()
            .any(|entry| matches!(entry.progress, LinkProgress::OwnId(_)))
    );
    guest.join(&ticket).expect("join");
    assert!(wait_until(|| matches!(
        guest.status().state,
        NetState::Connected { .. }
    )));
    let told: Vec<LinkProgress> = guest.progress().into_iter().map(|e| e.progress).collect();
    for wanted in [
        |p: &LinkProgress| matches!(p, LinkProgress::TicketRead { .. }),
        |p: &LinkProgress| matches!(p, LinkProgress::LookingUpHost),
        |p: &LinkProgress| {
            matches!(
                p,
                LinkProgress::HostFound {
                    by: LookupMeans::LocalNetwork,
                    ..
                }
            )
        },
        |p: &LinkProgress| matches!(p, LinkProgress::HostAnswered { .. }),
        |p: &LinkProgress| matches!(p, LinkProgress::HelloSent { .. }),
        |p: &LinkProgress| matches!(p, LinkProgress::WelcomeRead { seat: 1, .. }),
    ] {
        assert!(told.iter().any(wanted), "{told:?}");
    }
    let mut logged = Vec::new();
    assert!(wait_until(|| {
        logged.extend(guest.drain_events());
        logged.iter().any(|event| {
            matches!(event, LauncherEvent::LinkProgress { line, .. } if line.starts_with("welcome read from Hosty"))
        })
    }));
}

#[test]
fn a_join_whose_host_key_is_not_found_fails_with_its_own_text() {
    let host = local_core("Hosty", ANSWERED_ELSEWHERE_LOOKUP_PORT);
    let ticket = host.host().expect("ticket");
    let guest = local_core("Guesty", UNANSWERED_LOOKUP_PORT);
    guest.join(&ticket).expect("join");
    assert!(wait_until(|| matches!(
        guest.status().state,
        NetState::Failed(ref text) if text == HOST_KEY_NOT_FOUND_TEXT
    )));
    assert!(
        guest
            .progress()
            .iter()
            .any(|entry| matches!(entry.progress, LinkProgress::HostNotFound { .. }))
    );
}
