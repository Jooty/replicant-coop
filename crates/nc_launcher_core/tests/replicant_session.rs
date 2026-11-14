use std::thread;
use std::time::{Duration, Instant};

use nc_launcher_core::{LauncherConfig, LauncherCore, LauncherNet, NetState, judge_rejection_text};

use nc_net::{
    Channel, HANDSHAKE_TIMEOUT, KEEPALIVE_INTERVAL, LeaveReason, LoopbackNetwork,
    LoopbackTransport, PeerId, REFUSAL_LINGER, Role, Session, SessionConfig, SessionEvent,
    Transport, TransportEvent,
};
use nc_wire::{STORY_MODE, SessionRules};
use replicant_proto::{
    BuildGate, CombatEvent, EnemyGone, EnemyGoneReason, EnemyId, FollowPhase, Hello, Member,
    Message, PROTOCOL_VERSION, PlayerState, RejectReason, decode, encode,
};

use crate::bridge::{FakeGame, loopback_factory, wait_until};

const BUILD: BuildGate = BuildGate {
    time_date_stamp: 0x60B7_3C00,
    size_of_image: 0x0533_9000,
};
const OTHER_BUILD: BuildGate = BuildGate {
    time_date_stamp: 1,
    size_of_image: 2,
};

type LoopbackSession = Session<LoopbackTransport, Message>;

struct Pair {
    network: LoopbackNetwork,
    host: LoopbackSession,
    guest: LoopbackSession,
    host_events: Vec<SessionEvent<Message>>,
    guest_events: Vec<SessionEvent<Message>>,
    now: Instant,
}

fn config(role: Role, name: &str, build: BuildGate) -> SessionConfig {
    SessionConfig {
        role,
        name: name.to_owned(),
        build,
        rules: SessionRules {
            mode: STORY_MODE,
            max_players: 2,
        },
    }
}

fn state(tick: u32) -> PlayerState {
    PlayerState {
        tick,
        phase: 1,
        position: [0.0, 0.0, 0.0],
        yaw_radians: 0.0,
        motion: None,
        hp: 10,
        max_hp: 10,
        weapon: None,
        activity: Default::default(),
        mp: 0,
        max_mp: 0,
        level: 0,
        ridden_boar: None,
    }
}

impl Pair {
    fn new(guest_build: BuildGate) -> Self {
        let network = LoopbackNetwork::new();
        let mut host: LoopbackSession =
            Session::new(network.endpoint(), config(Role::Host, "Host", BUILD));
        let mut guest = Session::new(
            network.endpoint(),
            config(Role::Guest, "Guest", guest_build),
        );
        let invite = host.host().expect("invite");
        guest.join(&invite).expect("join");
        let mut pair = Self {
            network,
            host,
            guest,
            host_events: Vec::new(),
            guest_events: Vec::new(),
            now: Instant::now(),
        };
        pair.pump();
        pair
    }

    fn pump(&mut self) {
        for _ in 0..4 {
            self.host.poll(self.now, &mut self.host_events);
            self.guest.poll(self.now, &mut self.guest_events);
        }
    }

    fn advance(&mut self, by: Duration) {
        self.now += by;
        self.pump();
    }
}

#[test]
fn matching_builds_complete_the_handshake_and_assign_slots() {
    let pair = Pair::new(BUILD);
    assert!(pair.host_events.iter().any(|event| matches!(
        event,
        SessionEvent::PlayerJoined { name, slot: 1, .. } if name == "Guest"
    )));
    assert!(pair.guest_events.iter().any(|event| matches!(
        event,
        SessionEvent::PlayerJoined { name, slot: 0, .. } if name == "Host"
    )));
    assert_eq!(pair.guest.local_slot(), Some(1));
    assert_eq!(pair.host.ready_peers().len(), 1);
}

#[test]
fn a_different_build_is_rejected_on_both_sides() {
    let pair = Pair::new(OTHER_BUILD);
    assert!(pair.guest_events.contains(&SessionEvent::Rejected {
        reason: RejectReason::BuildMismatch
    }));
    assert!(pair.host_events.iter().any(|event| matches!(
        event,
        SessionEvent::PeerRefused {
            reason: RejectReason::BuildMismatch,
            ..
        }
    )));
    assert!(pair.host.ready_peers().is_empty());
}

#[test]
fn a_hello_from_another_protocol_version_is_rejected_with_a_reason() {
    let network = LoopbackNetwork::new();
    let mut host: LoopbackSession =
        Session::new(network.endpoint(), config(Role::Host, "Host", BUILD));
    let mut raw = network.endpoint();
    let invite = host.host().expect("invite");
    raw.join(&invite).expect("join");
    let hello = Message::Hello(Hello {
        version: PROTOCOL_VERSION + 1,
        build: BUILD,
        name: "Old".to_owned(),
    });
    let host_id = host.transport().local_peer();
    raw.send(
        host_id,
        Channel::ReliableOrdered,
        &encode(&hello).expect("encodes"),
    )
    .expect("sends");
    let mut events = Vec::new();
    host.poll(Instant::now(), &mut events);
    let mut replies = Vec::new();
    raw.poll(&mut replies);
    let reply = replies.iter().find_map(|event| match event {
        TransportEvent::Message { payload, .. } => decode(payload).ok(),
        _ => None,
    });
    assert_eq!(
        reply,
        Some(Message::Reject(replicant_proto::Reject {
            reason: RejectReason::VersionMismatch
        }))
    );
}

#[test]
fn a_second_guest_is_refused_because_the_session_is_full() {
    let mut pair = Pair::new(BUILD);
    let mut second: LoopbackSession = Session::new(
        pair.network.endpoint(),
        config(Role::Guest, "Second", BUILD),
    );
    let invite = pair.host.host().expect("invite");
    second.join(&invite).expect("join");
    let mut second_events = Vec::new();
    for _ in 0..4 {
        pair.host.poll(pair.now, &mut pair.host_events);
        second.poll(pair.now, &mut second_events);
    }
    assert!(second_events.contains(&SessionEvent::Rejected {
        reason: RejectReason::SessionFull
    }));
}

#[test]
fn states_arrive_and_older_ticks_are_dropped() {
    let mut pair = Pair::new(BUILD);
    for tick in [5, 4, 6] {
        pair.guest
            .send_ordered(&Message::State(state(tick)))
            .expect("valid state");
    }
    pair.pump();
    let ticks: Vec<u32> = pair
        .host_events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::Message {
                message: Message::State(state),
                ..
            } => Some(state.tick),
            _ => None,
        })
        .collect();
    assert_eq!(ticks, vec![5, 6]);
}

#[test]
fn combat_events_reach_the_other_player_in_order() {
    let mut pair = Pair::new(BUILD);
    let downed = Message::Combat(CombatEvent::Downed);
    let game_over = Message::Combat(CombatEvent::GameOver);
    pair.guest.send_relayed(&downed).expect("valid event");
    pair.guest.send_relayed(&game_over).expect("valid event");
    pair.pump();
    let events: Vec<Message> = pair
        .host_events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::Message { message, .. } => Some(message.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(events, vec![downed, game_over]);
}

#[test]
fn a_follow_phase_event_reaches_the_other_side() {
    let mut pair = Pair::new(BUILD);
    let event = Message::FollowPhase(FollowPhase::at(2, 1));
    pair.host.send_relayed(&event).expect("valid event");
    pair.pump();
    assert!(
        pair.guest_events
            .iter()
            .any(|seen| matches!(seen, SessionEvent::Message { message, .. } if *message == event))
    );
}

#[test]
fn an_invalid_state_is_not_sent() {
    let mut pair = Pair::new(BUILD);
    let bad = PlayerState { hp: 99, ..state(1) };
    assert!(pair.guest.send_ordered(&Message::State(bad)).is_err());
}

#[test]
fn keepalive_pings_measure_the_round_trip() {
    let mut pair = Pair::new(BUILD);
    pair.now += KEEPALIVE_INTERVAL;
    pair.host.poll(pair.now, &mut pair.host_events);
    pair.guest.poll(pair.now, &mut pair.guest_events);
    pair.now += Duration::from_millis(40);
    pair.host.poll(pair.now, &mut pair.host_events);
    let guest_peer = pair.host.ready_peers()[0];
    assert_eq!(pair.host.rtt(guest_peer), Some(Duration::from_millis(40)));
}

#[test]
fn a_silent_peer_times_out() {
    let mut pair = Pair::new(BUILD);
    pair.network.drop_all_messages(true);
    for _ in 0..8 {
        pair.advance(KEEPALIVE_INTERVAL);
    }
    assert!(pair.host_events.iter().any(|event| matches!(
        event,
        SessionEvent::PlayerLeft {
            reason: LeaveReason::TimedOut,
            ..
        }
    )));
}

#[test]
fn a_bye_ends_the_peer_cleanly() {
    let mut pair = Pair::new(BUILD);
    pair.guest.close();
    pair.pump();
    assert!(pair.host_events.iter().any(|event| matches!(
        event,
        SessionEvent::PlayerLeft {
            reason: LeaveReason::Quit,
            ..
        }
    )));
}

#[test]
fn a_refused_peer_is_dropped_after_the_linger() {
    let network = LoopbackNetwork::new();
    let mut host: LoopbackSession =
        Session::new(network.endpoint(), config(Role::Host, "Host", BUILD));
    let mut raw = network.endpoint();
    let invite = host.host().expect("invite");
    raw.join(&invite).expect("join");
    let hello = Message::Hello(Hello {
        version: PROTOCOL_VERSION,
        build: OTHER_BUILD,
        name: "Other".to_owned(),
    });
    let host_id = host.transport().local_peer();
    raw.send(
        host_id,
        Channel::ReliableOrdered,
        &encode(&hello).expect("encodes"),
    )
    .expect("sends");
    let start = Instant::now();
    let mut events = Vec::new();
    host.poll(start, &mut events);
    let mut before = Vec::new();
    raw.poll(&mut before);
    assert!(
        !before
            .iter()
            .any(|event| matches!(event, TransportEvent::PeerDisconnected(..)))
    );
    host.poll(
        start + REFUSAL_LINGER + Duration::from_millis(1),
        &mut events,
    );
    let mut after = Vec::new();
    raw.poll(&mut after);
    assert!(
        after
            .iter()
            .any(|event| matches!(event, TransportEvent::PeerDisconnected(..)))
    );
}

#[test]
fn garbage_before_the_handshake_drops_the_peer() {
    let network = LoopbackNetwork::new();
    let mut host: LoopbackSession =
        Session::new(network.endpoint(), config(Role::Host, "Host", BUILD));
    let mut raw = network.endpoint();
    let invite = host.host().expect("invite");
    raw.join(&invite).expect("join");
    let host_id = host.transport().local_peer();
    raw.send(host_id, Channel::ReliableOrdered, &[0xFF, 0xFF, 0xFF])
        .expect("sends");
    let mut events = Vec::new();
    host.poll(Instant::now(), &mut events);
    let mut raw_events = Vec::new();
    raw.poll(&mut raw_events);
    assert!(raw_events.iter().any(|event| matches!(
        event,
        TransportEvent::PeerDisconnected(peer, _) if *peer == host_id
    )));
}

#[test]
fn a_peer_that_never_says_hello_is_dropped_after_the_handshake_timeout() {
    let network = LoopbackNetwork::new();
    let mut host: LoopbackSession =
        Session::new(network.endpoint(), config(Role::Host, "Host", BUILD));
    let mut raw = network.endpoint();
    let invite = host.host().expect("invite");
    raw.join(&invite).expect("join");
    let start = Instant::now();
    let mut events = Vec::new();
    host.poll(start, &mut events);
    host.poll(
        start + HANDSHAKE_TIMEOUT + Duration::from_secs(1),
        &mut events,
    );
    let mut raw_events = Vec::new();
    raw.poll(&mut raw_events);
    let host_id: PeerId = host.transport().local_peer();
    assert!(raw_events.iter().any(|event| matches!(
        event,
        TransportEvent::PeerDisconnected(peer, _) if *peer == host_id
    )));
}

#[test]
fn relayed_game_messages_reach_the_other_side_unchanged() {
    let mut pair = Pair::new(BUILD);
    let gone = Message::EnemyGone(EnemyGone {
        phase: 2,
        id: EnemyId {
            group: 12,
            ordinal: 3,
        },
        reason: EnemyGoneReason::Killed,
    });
    pair.host.send_relayed(&gone).expect("a relayed message");
    pair.pump();
    assert!(pair.guest_events.iter().any(|event| matches!(
        event,
        SessionEvent::Message { message, .. } if *message == gone
    )));
}

#[test]
fn session_messages_cannot_be_sent_as_relayed() {
    let mut pair = Pair::new(BUILD);
    assert!(pair.host.send_relayed(&Message::Bye).is_err());
    assert!(pair.guest.send_relayed(&Message::State(state(1))).is_err());
}

#[test]
fn every_way_a_link_ends_has_its_own_words() {
    use nc_net::DisconnectReason;
    let labels = [
        LeaveReason::Quit.label(),
        LeaveReason::TimedOut.label(),
        LeaveReason::ProtocolViolation.label(),
        LeaveReason::TransportClosed.label(),
        DisconnectReason::Closed.label(),
        DisconnectReason::TimedOut.label(),
        DisconnectReason::Failed.label(),
    ];
    let distinct: std::collections::HashSet<_> = labels.iter().collect();
    assert_eq!(distinct.len(), labels.len());
}

fn launcher(network: &LoopbackNetwork, name: &str) -> LauncherCore {
    LauncherCore::new(LauncherConfig::new(name), loopback_factory(network)).expect("core")
}

#[test]
fn the_game_hears_member_joined_and_left() {
    let network = LoopbackNetwork::new();
    let host = launcher(&network, "Hosty");
    let guest = launcher(&network, "Guesty");
    let mut host_game = FakeGame::attach(&host, &LauncherConfig::new("x"));
    let ticket = host.host().expect("ticket");
    guest.join(&ticket).expect("join");
    let joined = Message::Member(Member {
        slot: 1,
        name: "Guesty".to_owned(),
    });
    assert!(wait_until(|| host_game.saw(|m| *m == joined)));
    let welcome = host_game
        .received
        .iter()
        .position(|m| matches!(m, Message::Welcome(w) if w.slot == 0));
    let member = host_game.received.iter().position(|m| *m == joined);
    assert!(welcome.is_some_and(|welcome| member.is_some_and(|member| welcome < member)));
    guest.leave();
    assert!(wait_until(
        || host_game.saw(|m| *m == Message::MemberLeft { slot: 1 })
    ));
    thread::sleep(Duration::from_millis(200));
    assert!(!host_game.saw(|m| *m == Message::Bye));
    assert!(matches!(host.status().state, NetState::Hosting { .. }));
}

#[test]
fn a_host_turns_away_a_third_launcher() {
    let network = LoopbackNetwork::new();
    let host = launcher(&network, "Hosty");
    let guest = launcher(&network, "Guesty");
    let third = launcher(&network, "Third");
    let mut host_game = FakeGame::attach(&host, &LauncherConfig::new("x"));
    let ticket = host.host().expect("ticket");
    guest.join(&ticket).expect("join");
    assert!(wait_until(|| matches!(
        guest.status().state,
        NetState::Connected { .. }
    )));
    third.join(&ticket).expect("join starts");
    let full = NetState::Failed(judge_rejection_text(RejectReason::SessionFull).to_owned());
    assert!(wait_until(|| third.status().state == full));
    assert!(judge_rejection_text(RejectReason::SessionFull).contains("full"));
    thread::sleep(Duration::from_millis(200));
    assert!(!host_game.saw(|m| matches!(m, Message::Member(member) if member.slot != 1)));
    assert_eq!(
        host.link_status()
            .partner()
            .map(|partner| partner.name.as_str()),
        Some("Guesty")
    );
    assert!(matches!(guest.status().state, NetState::Connected { .. }));
}

#[test]
fn the_game_hears_nothing_from_a_player_before_their_member_or_after_they_left() {
    let network = LoopbackNetwork::new();
    let host = launcher(&network, "Hosty");
    let guest = launcher(&network, "Guesty");
    let mut host_game = FakeGame::attach(&host, &LauncherConfig::new("x"));
    let mut guest_game = FakeGame::attach(&guest, &LauncherConfig::new("x"));
    assert!(wait_until(|| guest.link_status().game_linked));
    let ticket = host.host().expect("ticket");
    guest.join(&ticket).expect("join");
    let mut tick = 0;
    assert!(wait_until(|| {
        tick += 1;
        guest_game.send_state(&crate::bridge::state_at(tick, 2.0));
        host_game.saw(|m| matches!(m, Message::State(_)))
    }));
    guest.leave();
    assert!(wait_until(
        || host_game.saw(|m| *m == Message::MemberLeft { slot: 1 })
    ));
    for _ in 0..20 {
        tick += 1;
        guest_game.send_state(&crate::bridge::state_at(tick, 2.0));
        thread::sleep(Duration::from_millis(10));
    }
    host_game.pump();
    let joined = host_game
        .received
        .iter()
        .position(|m| matches!(m, Message::Member(member) if member.slot == 1))
        .expect("member announced");
    let left = host_game
        .received
        .iter()
        .position(|m| *m == Message::MemberLeft { slot: 1 })
        .expect("member left");
    let from_guest: Vec<usize> = host_game
        .envelopes
        .iter()
        .enumerate()
        .filter(|(_, envelope)| envelope.from == 1)
        .map(|(index, _)| index)
        .collect();
    assert!(!from_guest.is_empty());
    assert!(
        from_guest
            .iter()
            .all(|index| *index > joined && *index < left)
    );
}
