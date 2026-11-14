use std::time::{Duration, Instant};

use nc_net::{
    Channel, LeaveReason, LinkProgress, LoopbackNetwork, LoopbackTransport, PeerId, ProgressBoard,
    Role, Session, SessionConfig, SessionEvent, Transport, TransportEvent,
};
use nc_wire::{
    BuildGate, Envelope, GameMessage, Hello, LinkPayload, MAX_SESSION_SLOTS, Member, MemberPing,
    Ping, Pong, Reject, RejectReason, STORY_MODE, SessionRules, Welcome, WireError, decode,
    decode_enveloped, encode, encode_enveloped,
};
use serde::{Deserialize, Serialize};

const BUILD: BuildGate = BuildGate {
    time_date_stamp: 7,
    size_of_image: 9,
};
const THREE_PLAYERS: u8 = 3;
const TWO_PLAYERS: u8 = 2;
const MAX_CHAT_BYTES: usize = 16;
const GUEST_ENVELOPE: Envelope = Envelope { from: 1, to: None };

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum Msg<const VERSION: u16, const PLAYERS: u8> {
    Hello(Hello),
    Welcome(Welcome),
    Reject(Reject),
    Ping(Ping),
    Pong(Pong),
    Bye,
    Position { tick: u32, x: i32 },
    Chat(String),
    LocalOnly(u8),
    Whisper(String),
    Shout(String),
    Member(Member),
    MemberLeft { slot: u8 },
    Rules(SessionRules),
    MemberPings(Vec<MemberPing>),
}

impl<const VERSION: u16, const PLAYERS: u8> GameMessage for Msg<VERSION, PLAYERS> {
    const PROTOCOL_VERSION: u16 = VERSION;
    const MAX_PLAYERS: u8 = PLAYERS;

    fn from_link(payload: LinkPayload) -> Self {
        match payload {
            LinkPayload::Hello(hello) => Msg::Hello(hello),
            LinkPayload::Welcome(welcome) => Msg::Welcome(welcome),
            LinkPayload::Reject(reject) => Msg::Reject(reject),
            LinkPayload::Ping(ping) => Msg::Ping(ping),
            LinkPayload::Pong(pong) => Msg::Pong(pong),
            LinkPayload::Bye => Msg::Bye,
            LinkPayload::Member(member) => Msg::Member(member),
            LinkPayload::MemberLeft { slot } => Msg::MemberLeft { slot },
            LinkPayload::Rules(rules) => Msg::Rules(rules),
            LinkPayload::MemberPings(pings) => Msg::MemberPings(pings),
        }
    }

    fn into_link(self) -> Result<LinkPayload, Self> {
        match self {
            Msg::Hello(hello) => Ok(LinkPayload::Hello(hello)),
            Msg::Welcome(welcome) => Ok(LinkPayload::Welcome(welcome)),
            Msg::Reject(reject) => Ok(LinkPayload::Reject(reject)),
            Msg::Ping(ping) => Ok(LinkPayload::Ping(ping)),
            Msg::Pong(pong) => Ok(LinkPayload::Pong(pong)),
            Msg::Bye => Ok(LinkPayload::Bye),
            Msg::Member(member) => Ok(LinkPayload::Member(member)),
            Msg::MemberLeft { slot } => Ok(LinkPayload::MemberLeft { slot }),
            Msg::Rules(rules) => Ok(LinkPayload::Rules(rules)),
            Msg::MemberPings(pings) => Ok(LinkPayload::MemberPings(pings)),
            other => Err(other),
        }
    }

    fn is_relayed(&self) -> bool {
        matches!(self, Msg::Chat(_) | Msg::Whisper(_) | Msg::Shout(_))
    }

    fn forwarded_to_other_guests(&self) -> bool {
        matches!(self, Msg::Shout(_) | Msg::Position { .. })
    }

    fn is_latest_wins(&self) -> bool {
        matches!(self, Msg::Position { .. })
    }

    fn ordered_tick(&self) -> Option<u32> {
        match self {
            Msg::Position { tick, .. } => Some(*tick),
            _ => None,
        }
    }

    fn validate(&self) -> Result<(), WireError> {
        match self {
            Msg::Hello(hello) if hello.name.is_empty() => {
                Err(WireError::OutOfRange { field: "name" })
            }
            Msg::Chat(text) if text.len() > MAX_CHAT_BYTES => {
                Err(WireError::OutOfRange { field: "chat" })
            }
            _ => Ok(()),
        }
    }
}

type Current = Msg<4, THREE_PLAYERS>;
type Older = Msg<3, THREE_PLAYERS>;
type Pair = Msg<4, TWO_PLAYERS>;
type Crowd = Msg<4, MAX_SESSION_SLOTS>;

fn rules_for(max_players: u8) -> SessionRules {
    SessionRules {
        mode: STORY_MODE,
        max_players,
    }
}

fn config_for(role: Role, name: &str, max_players: u8) -> SessionConfig {
    SessionConfig {
        role,
        name: name.to_owned(),
        build: BUILD,
        rules: rules_for(max_players),
    }
}

fn config(role: Role, name: &str) -> SessionConfig {
    config_for(role, name, THREE_PLAYERS)
}

fn pump<M: GameMessage>(
    sessions: &mut [(
        &mut Session<LoopbackTransport, M>,
        &mut Vec<SessionEvent<M>>,
    )],
) {
    let now = Instant::now();
    for _ in 0..6 {
        for (session, events) in sessions.iter_mut() {
            session.poll(now, events);
        }
    }
}

fn relayed_chats<const VERSION: u16, const PLAYERS: u8>(
    events: &[SessionEvent<Msg<VERSION, PLAYERS>>],
) -> Vec<(u8, String)> {
    events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::Message {
                from,
                message: Msg::Chat(text),
            } => Some((*from, text.clone())),
            _ => None,
        })
        .collect()
}

struct Trio {
    network: LoopbackNetwork,
    host: Session<LoopbackTransport, Current>,
    first: Session<LoopbackTransport, Current>,
    second: Session<LoopbackTransport, Current>,
    host_events: Vec<SessionEvent<Current>>,
    first_events: Vec<SessionEvent<Current>>,
    second_events: Vec<SessionEvent<Current>>,
}

impl Trio {
    fn connected() -> Self {
        let network = LoopbackNetwork::new();
        let mut host = Session::new(network.endpoint(), config(Role::Host, "Host"));
        let mut first = Session::new(network.endpoint(), config(Role::Guest, "First"));
        let mut second = Session::new(network.endpoint(), config(Role::Guest, "Second"));
        let invite = host.host().expect("invite");
        first.join(&invite).expect("join");
        second.join(&invite).expect("join");
        let mut trio = Self {
            network,
            host,
            first,
            second,
            host_events: Vec::new(),
            first_events: Vec::new(),
            second_events: Vec::new(),
        };
        trio.pump();
        trio
    }

    fn pump(&mut self) {
        pump(&mut [
            (&mut self.host, &mut self.host_events),
            (&mut self.first, &mut self.first_events),
            (&mut self.second, &mut self.second_events),
        ]);
    }
}

#[test]
fn a_host_and_two_guests_complete_the_handshake_with_their_own_slots() {
    let trio = Trio::connected();
    let slots: Vec<u8> = trio
        .host_events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::PlayerJoined { slot, .. } => Some(*slot),
            _ => None,
        })
        .collect();
    assert_eq!(slots, vec![1, 2]);
    assert_eq!(trio.first.local_slot(), Some(1));
    assert_eq!(trio.second.local_slot(), Some(2));
    assert_eq!(trio.host.ready_peers().len(), 2);
}

#[test]
fn a_relayed_kind_reaches_the_host_from_each_guest() {
    let mut trio = Trio::connected();
    trio.first
        .send_relayed(&Msg::Chat("one".to_owned()))
        .expect("relayed kind");
    trio.second
        .send_relayed(&Msg::Chat("two".to_owned()))
        .expect("relayed kind");
    trio.pump();
    let mut texts: Vec<String> = relayed_chats(&trio.host_events)
        .into_iter()
        .map(|(_, text)| text)
        .collect();
    texts.sort();
    assert_eq!(texts, vec!["one".to_owned(), "two".to_owned()]);
    assert!(relayed_chats(&trio.second_events).is_empty());
}

#[test]
fn a_local_kind_is_not_sent_and_not_accepted_from_a_peer() {
    let mut trio = Trio::connected();
    assert!(trio.first.send_relayed(&Msg::LocalOnly(1)).is_err());
    trio.pump();
    assert!(
        !trio
            .host_events
            .iter()
            .any(|event| matches!(event, SessionEvent::Message { .. }))
    );
}

#[test]
fn a_local_kind_from_a_raw_peer_ends_that_peer_as_a_violation() {
    let network = LoopbackNetwork::new();
    let mut host: Session<LoopbackTransport, Current> =
        Session::new(network.endpoint(), config(Role::Host, "Host"));
    let mut raw = network.endpoint();
    let invite = host.host().expect("invite");
    raw.join(&invite).expect("join");
    let host_id = host.transport().local_peer();
    let hello = Current::Hello(Hello {
        version: 4,
        build: BUILD,
        name: "Raw".to_owned(),
    });
    let mut events = Vec::new();
    let now = Instant::now();
    let local_only = encode_enveloped(&GUEST_ENVELOPE, &Current::LocalOnly(3)).expect("encodes");
    for frame in [encode(&hello).expect("encodes"), local_only] {
        raw.send(host_id, Channel::ReliableOrdered, &frame)
            .expect("sends");
        host.poll(now, &mut events);
    }
    assert!(events.iter().any(|event| matches!(
        event,
        SessionEvent::PlayerLeft {
            reason: LeaveReason::ProtocolViolation,
            ..
        }
    )));
}

#[test]
fn ordered_messages_arrive_and_older_ticks_are_dropped() {
    let mut trio = Trio::connected();
    for tick in [5, 4, 6] {
        trio.first
            .send_ordered(&Msg::Position { tick, x: 0 })
            .expect("ordered kind");
    }
    trio.pump();
    let ticks: Vec<u32> = trio
        .host_events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::Message {
                message: Msg::Position { tick, .. },
                ..
            } => Some(*tick),
            _ => None,
        })
        .collect();
    assert_eq!(ticks, vec![5, 6]);
    assert!(trio.first.send_ordered(&Msg::Chat("x".to_owned())).is_err());
}

#[test]
fn an_invalid_message_is_not_sent() {
    let mut trio = Trio::connected();
    let too_long = Msg::Chat("x".repeat(MAX_CHAT_BYTES + 1));
    assert!(trio.first.send_relayed(&too_long).is_err());
}

#[test]
fn a_different_protocol_version_is_rejected_on_both_sides() {
    let network = LoopbackNetwork::new();
    let mut host: Session<LoopbackTransport, Current> =
        Session::new(network.endpoint(), config(Role::Host, "Host"));
    let mut guest: Session<LoopbackTransport, Older> =
        Session::new(network.endpoint(), config(Role::Guest, "Guest"));
    let invite = host.host().expect("invite");
    guest.join(&invite).expect("join");
    let mut host_events = Vec::new();
    let mut guest_events = Vec::new();
    let now = Instant::now();
    for _ in 0..6 {
        host.poll(now, &mut host_events);
        guest.poll(now, &mut guest_events);
    }
    assert!(guest_events.contains(&SessionEvent::Rejected {
        reason: RejectReason::VersionMismatch
    }));
    assert!(host_events.iter().any(|event| matches!(
        event,
        SessionEvent::PeerRefused {
            reason: RejectReason::VersionMismatch,
            ..
        }
    )));
    assert!(host.ready_peers().is_empty());
}

#[test]
fn a_third_guest_is_refused_because_the_session_is_full() {
    let mut trio = Trio::connected();
    let mut late: Session<LoopbackTransport, Current> =
        Session::new(trio.network.endpoint(), config(Role::Guest, "Late"));
    let invite = trio.host.host().expect("invite");
    late.join(&invite).expect("join");
    let mut late_events = Vec::new();
    for _ in 0..4 {
        trio.pump();
        late.poll(Instant::now(), &mut late_events);
    }
    assert!(late_events.contains(&SessionEvent::Rejected {
        reason: RejectReason::SessionFull
    }));
}

#[test]
fn a_bye_ends_the_peer_cleanly() {
    let mut trio = Trio::connected();
    trio.second.close();
    trio.pump();
    assert!(trio.host_events.iter().any(|event| matches!(
        event,
        SessionEvent::PlayerLeft {
            reason: LeaveReason::Quit,
            ..
        }
    )));
}

fn pair_session(
    network: &LoopbackNetwork,
    role: Role,
    name: &str,
) -> Session<LoopbackTransport, Pair> {
    Session::new(network.endpoint(), config_for(role, name, TWO_PLAYERS))
}

fn raw_guest_says_hello(
    network: &LoopbackNetwork,
    host: &mut Session<LoopbackTransport, Pair>,
    version: u16,
    host_events: &mut Vec<SessionEvent<Pair>>,
) -> (LoopbackTransport, PeerId) {
    let mut raw = network.endpoint();
    let invite = host.host().expect("invite");
    raw.join(&invite).expect("join");
    let hello = Pair::Hello(Hello {
        version,
        build: BUILD,
        name: "Raw".to_owned(),
    });
    let host_id = host.transport().local_peer();
    raw.send(
        host_id,
        Channel::ReliableOrdered,
        &encode(&hello).expect("encodes"),
    )
    .expect("sends");
    host.poll(Instant::now(), host_events);
    (raw, host_id)
}

fn frames_received(raw: &mut LoopbackTransport) -> Vec<Vec<u8>> {
    let mut events = Vec::new();
    raw.poll(&mut events);
    events
        .into_iter()
        .filter_map(|event| match event {
            TransportEvent::Message { payload, .. } => Some(payload),
            _ => None,
        })
        .collect()
}

struct Duo {
    host: Session<LoopbackTransport, Pair>,
    guest: Session<LoopbackTransport, Pair>,
    host_events: Vec<SessionEvent<Pair>>,
    guest_events: Vec<SessionEvent<Pair>>,
}

impl Duo {
    fn connected(network: &LoopbackNetwork) -> Self {
        let mut host = pair_session(network, Role::Host, "Host");
        let mut guest = pair_session(network, Role::Guest, "Guest");
        let invite = host.host().expect("invite");
        guest.join(&invite).expect("join");
        let mut duo = Self {
            host,
            guest,
            host_events: Vec::new(),
            guest_events: Vec::new(),
        };
        duo.pump();
        duo
    }

    fn pump(&mut self) {
        pump(&mut [
            (&mut self.host, &mut self.host_events),
            (&mut self.guest, &mut self.guest_events),
        ]);
    }
}

#[test]
fn a_session_refuses_a_joiner_beyond_the_cap() {
    let network = LoopbackNetwork::new();
    let mut duo = Duo::connected(&network);
    let mut late = pair_session(&network, Role::Guest, "Late");
    let invite = duo.host.host().expect("invite");
    late.join(&invite).expect("join");
    let mut late_events = Vec::new();
    for _ in 0..4 {
        duo.pump();
        late.poll(Instant::now(), &mut late_events);
    }
    assert!(late_events.contains(&SessionEvent::Rejected {
        reason: RejectReason::SessionFull
    }));
    assert!(duo.host_events.iter().any(|event| matches!(
        event,
        SessionEvent::PeerRefused {
            reason: RejectReason::SessionFull,
            ..
        }
    )));
    assert_eq!(duo.host.ready_peers().len(), 1);
    assert_eq!(duo.guest.local_slot(), Some(1));
    assert_eq!(late.local_slot(), None);
}

#[test]
fn two_players_see_each_other_join_by_slot_and_name() {
    let network = LoopbackNetwork::new();
    let duo = Duo::connected(&network);
    assert!(duo.host_events.contains(&SessionEvent::PlayerJoined {
        slot: 1,
        name: "Guest".to_owned(),
        peer: duo.guest.transport().local_peer(),
    }));
    assert!(duo.guest_events.contains(&SessionEvent::PlayerJoined {
        slot: 0,
        name: "Host".to_owned(),
        peer: duo.host.transport().local_peer(),
    }));
    assert_eq!(duo.host.local_slot(), Some(0));
    assert_eq!(duo.guest.local_slot(), Some(1));
}

#[test]
fn a_host_renamed_before_anyone_joins_is_known_by_the_new_name() {
    let network = LoopbackNetwork::new();
    let mut host = pair_session(&network, Role::Host, "Host");
    host.set_name("Renamed");
    let mut guest = pair_session(&network, Role::Guest, "Guest");
    let invite = host.host().expect("invite");
    guest.join(&invite).expect("join");
    let mut duo = Duo {
        host,
        guest,
        host_events: Vec::new(),
        guest_events: Vec::new(),
    };
    duo.pump();
    assert!(duo.guest_events.contains(&SessionEvent::PlayerJoined {
        slot: 0,
        name: "Renamed".to_owned(),
        peer: duo.host.transport().local_peer(),
    }));
}

#[test]
fn a_guest_that_leaves_is_reported_by_its_slot() {
    let network = LoopbackNetwork::new();
    let mut duo = Duo::connected(&network);
    duo.guest.close();
    duo.pump();
    assert!(duo.host_events.iter().any(|event| matches!(
        event,
        SessionEvent::PlayerLeft {
            slot: 1,
            reason: LeaveReason::Quit,
            ..
        }
    )));
}

#[test]
fn a_host_that_leaves_is_reported_as_slot_zero() {
    let network = LoopbackNetwork::new();
    let mut duo = Duo::connected(&network);
    duo.host.close();
    duo.pump();
    assert!(duo.guest_events.iter().any(|event| matches!(
        event,
        SessionEvent::PlayerLeft {
            slot: 0,
            reason: LeaveReason::Quit,
            ..
        }
    )));
}

#[test]
fn messages_carry_the_sender_slot_both_ways() {
    let network = LoopbackNetwork::new();
    let mut duo = Duo::connected(&network);
    duo.guest
        .send_relayed(&Msg::Chat("up".to_owned()))
        .expect("relayed kind");
    duo.host
        .send_relayed(&Msg::Chat("down".to_owned()))
        .expect("relayed kind");
    duo.pump();
    assert_eq!(relayed_chats(&duo.host_events), vec![(1, "up".to_owned())]);
    assert_eq!(
        relayed_chats(&duo.guest_events),
        vec![(0, "down".to_owned())]
    );
}

#[test]
fn a_guest_cannot_send_as_another_slot() {
    let network = LoopbackNetwork::new();
    let mut host = pair_session(&network, Role::Host, "Host");
    let mut events = Vec::new();
    let (mut raw, host_id) = raw_guest_says_hello(&network, &mut host, 4, &mut events);
    let posing_as_the_host = Envelope { from: 0, to: None };
    let frame =
        encode_enveloped(&posing_as_the_host, &Pair::Chat("spoof".to_owned())).expect("encodes");
    raw.send(host_id, Channel::ReliableOrdered, &frame)
        .expect("sends");
    host.poll(Instant::now(), &mut events);
    assert_eq!(relayed_chats(&events), vec![(1, "spoof".to_owned())]);
}

#[test]
fn the_handshake_is_bare_and_later_frames_carry_the_envelope() {
    let network = LoopbackNetwork::new();
    let mut host = pair_session(&network, Role::Host, "Host");
    let mut events = Vec::new();
    let (mut raw, _) = raw_guest_says_hello(&network, &mut host, 4, &mut events);
    let handshake = frames_received(&mut raw);
    assert!(matches!(
        decode::<Pair>(&handshake[0]),
        Ok(Msg::Welcome(Welcome { slot: 1, .. }))
    ));
    host.send_relayed(&Msg::Chat("hi".to_owned()))
        .expect("relayed kind");
    let ready: Vec<_> = frames_received(&mut raw)
        .iter()
        .filter_map(|frame| decode_enveloped::<Pair>(frame).ok())
        .collect();
    assert!(ready.contains(&(Envelope { from: 0, to: None }, Msg::Chat("hi".to_owned()))));
}

#[test]
fn a_frame_for_another_slot_is_not_taken() {
    let network = LoopbackNetwork::new();
    let mut host = pair_session(&network, Role::Host, "Host");
    let mut events = Vec::new();
    let (mut raw, host_id) = raw_guest_says_hello(&network, &mut host, 4, &mut events);
    let for_the_guest_itself = Envelope {
        from: 1,
        to: Some(1),
    };
    let frame =
        encode_enveloped(&for_the_guest_itself, &Pair::Chat("x".to_owned())).expect("encodes");
    raw.send(host_id, Channel::ReliableOrdered, &frame)
        .expect("sends");
    host.poll(Instant::now(), &mut events);
    assert!(relayed_chats(&events).is_empty());
    assert_eq!(host.ready_peers().len(), 1);
}

#[test]
fn an_old_version_is_refused_cleanly() {
    let network = LoopbackNetwork::new();
    let mut host = pair_session(&network, Role::Host, "Host");
    let mut events = Vec::new();
    let (mut raw, _) = raw_guest_says_hello(&network, &mut host, 3, &mut events);
    let mut raw_events = Vec::new();
    raw.poll(&mut raw_events);
    assert!(raw_events.iter().any(|event| matches!(
        event,
        TransportEvent::Message { payload, .. }
            if decode::<Older>(payload)
                == Ok(Msg::Reject(Reject { reason: RejectReason::VersionMismatch }))
    )));
    assert!(
        !raw_events
            .iter()
            .any(|event| matches!(event, TransportEvent::PeerDisconnected(..)))
    );
    assert_eq!(host.refused_peer_version(), Some(3));
    assert!(events.iter().any(|event| matches!(
        event,
        SessionEvent::PeerRefused {
            reason: RejectReason::VersionMismatch,
            ..
        }
    )));
}

struct Gathering {
    network: LoopbackNetwork,
    host: Session<LoopbackTransport, Crowd>,
    host_events: Vec<SessionEvent<Crowd>>,
    guests: Vec<(Session<LoopbackTransport, Crowd>, Vec<SessionEvent<Crowd>>)>,
}

impl Gathering {
    fn with_cap(max_players: u8) -> Self {
        let network = LoopbackNetwork::new();
        let host = Session::new(
            network.endpoint(),
            config_for(Role::Host, "Host", max_players),
        );
        Self {
            network,
            host,
            host_events: Vec::new(),
            guests: Vec::new(),
        }
    }

    fn join_guests(&mut self, count: usize) {
        let invite = self.host.host().expect("invite");
        for index in 0..count {
            let name = format!("Guest{index}");
            let mut guest = Session::new(
                self.network.endpoint(),
                config_for(Role::Guest, &name, MAX_SESSION_SLOTS),
            );
            guest.join(&invite).expect("join");
            self.guests.push((guest, Vec::new()));
            self.pump();
        }
    }

    fn pump(&mut self) {
        self.pump_at(Instant::now());
    }

    fn pump_at(&mut self, now: Instant) {
        for _ in 0..6 {
            self.host.poll(now, &mut self.host_events);
            for (guest, events) in &mut self.guests {
                guest.poll(now, events);
            }
        }
    }

    fn refused_guests(&self) -> usize {
        self.guests
            .iter()
            .filter(|(_, events)| {
                events.contains(&SessionEvent::Rejected {
                    reason: RejectReason::SessionFull,
                })
            })
            .count()
    }
}

#[test]
fn a_session_of_sixteen_admits_fifteen_guests() {
    let mut gathering = Gathering::with_cap(MAX_SESSION_SLOTS);
    gathering.join_guests(15);
    assert_eq!(gathering.host.ready_peers().len(), 15);
    let mut slots: Vec<u8> = gathering
        .guests
        .iter()
        .filter_map(|(guest, _)| guest.local_slot())
        .collect();
    slots.sort_unstable();
    assert_eq!(slots, (1..MAX_SESSION_SLOTS).collect::<Vec<u8>>());
    gathering.join_guests(1);
    assert_eq!(gathering.refused_guests(), 1);
    assert_eq!(gathering.host.ready_peers().len(), 15);
}

#[test]
fn a_joiner_beyond_the_hosts_cap_is_refused() {
    let mut gathering = Gathering::with_cap(3);
    gathering.join_guests(3);
    assert_eq!(gathering.host.ready_peers().len(), 2);
    assert_eq!(gathering.refused_guests(), 1);
    assert!(gathering.host_events.iter().any(|event| matches!(
        event,
        SessionEvent::PeerRefused {
            reason: RejectReason::SessionFull,
            ..
        }
    )));
}

#[test]
fn a_cap_above_the_games_ceiling_is_clamped() {
    let network = LoopbackNetwork::new();
    let mut host: Session<LoopbackTransport, Current> = Session::new(
        network.endpoint(),
        config_for(Role::Host, "Host", MAX_SESSION_SLOTS),
    );
    assert_eq!(host.rules().max_players, THREE_PLAYERS);
    host.set_rules(rules_for(MAX_SESSION_SLOTS));
    assert_eq!(host.rules().max_players, THREE_PLAYERS);
    host.set_rules(rules_for(1));
    assert_eq!(host.rules().max_players, TWO_PLAYERS);
}

#[test]
fn lowering_the_cap_keeps_members_and_refuses_new_ones() {
    let mut gathering = Gathering::with_cap(5);
    gathering.join_guests(3);
    gathering.host.set_rules(rules_for(2));
    gathering.pump();
    assert_eq!(gathering.host.rules().max_players, 2);
    assert_eq!(gathering.host.ready_peers().len(), 3);
    gathering.join_guests(1);
    assert_eq!(gathering.refused_guests(), 1);
    assert_eq!(gathering.host.ready_peers().len(), 3);
}

#[test]
fn a_guest_learns_changed_rules() {
    let mut gathering = Gathering::with_cap(6);
    gathering.join_guests(2);
    assert_eq!(gathering.guests[0].0.rules(), rules_for(6));
    let changed = SessionRules {
        mode: 3,
        max_players: 4,
    };
    gathering.host.set_rules(changed);
    gathering.pump();
    for (guest, events) in &gathering.guests {
        assert_eq!(guest.rules(), changed);
        assert!(events.contains(&SessionEvent::RulesChanged(changed)));
    }
}

#[test]
fn rules_from_a_guest_end_that_guest() {
    let network = LoopbackNetwork::new();
    let mut host = pair_session(&network, Role::Host, "Host");
    let mut events = Vec::new();
    let (mut raw, host_id) = raw_guest_says_hello(&network, &mut host, 4, &mut events);
    let frame = encode_enveloped(&GUEST_ENVELOPE, &Pair::Rules(rules_for(2))).expect("encodes");
    raw.send(host_id, Channel::ReliableOrdered, &frame)
        .expect("sends");
    host.poll(Instant::now(), &mut events);
    assert!(events.iter().any(|event| matches!(
        event,
        SessionEvent::PlayerLeft {
            reason: LeaveReason::ProtocolViolation,
            ..
        }
    )));
    assert!(host.ready_peers().is_empty());
    assert_eq!(host.rules(), rules_for(2));
}

fn positions_seen(events: &[SessionEvent<Crowd>]) -> Vec<(u8, u32)> {
    events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::Message {
                from,
                message: Msg::Position { tick, .. },
            } => Some((*from, *tick)),
            _ => None,
        })
        .collect()
}

fn shouts_seen(events: &[SessionEvent<Crowd>]) -> Vec<(u8, String)> {
    events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::Message {
                from,
                message: Msg::Shout(text),
            } => Some((*from, text.clone())),
            _ => None,
        })
        .collect()
}

fn whispers_seen(events: &[SessionEvent<Crowd>]) -> Vec<(u8, String)> {
    events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::Message {
                from,
                message: Msg::Whisper(text),
            } => Some((*from, text.clone())),
            _ => None,
        })
        .collect()
}

fn gathering_of(guests: usize) -> Gathering {
    let mut gathering = Gathering::with_cap(MAX_SESSION_SLOTS);
    gathering.join_guests(guests);
    gathering
}

#[test]
fn a_forwarded_kind_reaches_every_other_guest_with_its_sender() {
    let mut gathering = gathering_of(3);
    gathering.guests[0]
        .0
        .send_relayed(&Msg::Shout("hello".to_owned()))
        .expect("relayed kind");
    gathering.guests[0]
        .0
        .send_ordered(&Msg::Position { tick: 1, x: 5 })
        .expect("ordered kind");
    gathering.pump();
    assert_eq!(
        shouts_seen(&gathering.host_events),
        vec![(1, "hello".to_owned())]
    );
    assert_eq!(positions_seen(&gathering.host_events), vec![(1, 1)]);
    for index in [1, 2] {
        let events = &gathering.guests[index].1;
        assert_eq!(shouts_seen(events), vec![(1, "hello".to_owned())]);
        assert_eq!(positions_seen(events), vec![(1, 1)]);
    }
    assert!(shouts_seen(&gathering.guests[0].1).is_empty());
    assert!(positions_seen(&gathering.guests[0].1).is_empty());
}

#[test]
fn a_kind_not_forwarded_stays_at_the_host() {
    let mut gathering = gathering_of(2);
    gathering.guests[0]
        .0
        .send_relayed(&Msg::Chat("psst".to_owned()))
        .expect("relayed kind");
    gathering.pump();
    assert_eq!(
        relayed_chats(&gathering.host_events),
        vec![(1, "psst".to_owned())]
    );
    assert!(relayed_chats(&gathering.guests[1].1).is_empty());
}

#[test]
fn states_of_two_senders_do_not_drop_each_other() {
    let mut gathering = gathering_of(3);
    gathering.guests[0]
        .0
        .send_ordered(&Msg::Position { tick: 10, x: 1 })
        .expect("ordered kind");
    gathering.guests[1]
        .0
        .send_ordered(&Msg::Position { tick: 3, x: 2 })
        .expect("ordered kind");
    gathering.pump();
    gathering.guests[0]
        .0
        .send_ordered(&Msg::Position { tick: 11, x: 1 })
        .expect("ordered kind");
    gathering.guests[1]
        .0
        .send_ordered(&Msg::Position { tick: 4, x: 2 })
        .expect("ordered kind");
    gathering.pump();
    let mut seen = positions_seen(&gathering.guests[2].1);
    seen.sort_unstable();
    assert_eq!(seen, vec![(1, 10), (1, 11), (2, 3), (2, 4)]);
    gathering.guests[0]
        .0
        .send_ordered(&Msg::Position { tick: 9, x: 1 })
        .expect("ordered kind");
    gathering.pump();
    assert_eq!(positions_seen(&gathering.guests[2].1).len(), 4);
}

#[test]
fn an_addressed_message_reaches_only_that_guest_and_not_the_hosts_game() {
    let mut gathering = gathering_of(3);
    gathering.guests[0]
        .0
        .send_relayed_to(3, &Msg::Whisper("for three".to_owned()))
        .expect("relayed kind");
    gathering.guests[0]
        .0
        .send_ordered_to(3, &Msg::Position { tick: 1, x: 0 })
        .expect("ordered kind");
    gathering.pump();
    assert_eq!(
        whispers_seen(&gathering.guests[2].1),
        vec![(1, "for three".to_owned())]
    );
    assert_eq!(positions_seen(&gathering.guests[2].1), vec![(1, 1)]);
    for events in [
        &gathering.host_events,
        &gathering.guests[0].1,
        &gathering.guests[1].1,
    ] {
        assert!(whispers_seen(events).is_empty());
        assert!(positions_seen(events).is_empty());
    }
}

#[test]
fn a_message_to_an_absent_slot_is_dropped() {
    let mut gathering = gathering_of(2);
    gathering.guests[0]
        .0
        .send_relayed_to(9, &Msg::Whisper("nobody".to_owned()))
        .expect("relayed kind");
    gathering.pump();
    for events in [
        &gathering.host_events,
        &gathering.guests[0].1,
        &gathering.guests[1].1,
    ] {
        assert!(whispers_seen(events).is_empty());
    }
    assert_eq!(gathering.host.ready_peers().len(), 2);
}

#[test]
fn a_guest_cannot_choose_its_sender_slot() {
    let mut gathering = gathering_of(2);
    let invite = gathering.host.host().expect("invite");
    let mut raw = gathering.network.endpoint();
    raw.join(&invite).expect("join");
    let hello = Crowd::Hello(Hello {
        version: 4,
        build: BUILD,
        name: "Raw".to_owned(),
    });
    let host_id = gathering.host.transport().local_peer();
    raw.send(
        host_id,
        Channel::ReliableOrdered,
        &encode(&hello).expect("encodes"),
    )
    .expect("sends");
    gathering.pump();
    let posing_as_two = Envelope { from: 2, to: None };
    let frame =
        encode_enveloped(&posing_as_two, &Crowd::Shout("spoof".to_owned())).expect("encodes");
    raw.send(host_id, Channel::ReliableOrdered, &frame)
        .expect("sends");
    gathering.pump();
    assert_eq!(
        shouts_seen(&gathering.host_events),
        vec![(3, "spoof".to_owned())]
    );
    assert_eq!(
        shouts_seen(&gathering.guests[0].1),
        vec![(3, "spoof".to_owned())]
    );
    let addressed_as_two = Envelope {
        from: 2,
        to: Some(1),
    };
    let frame =
        encode_enveloped(&addressed_as_two, &Crowd::Whisper("spoof".to_owned())).expect("encodes");
    raw.send(host_id, Channel::ReliableOrdered, &frame)
        .expect("sends");
    gathering.pump();
    assert_eq!(
        whispers_seen(&gathering.guests[0].1),
        vec![(3, "spoof".to_owned())]
    );
}

#[test]
fn sixteen_players_each_receive_the_other_fifteen() {
    let mut gathering = gathering_of(15);
    for index in 0..15 {
        gathering.guests[index]
            .0
            .send_ordered(&Msg::Position { tick: 1, x: 0 })
            .expect("ordered kind");
    }
    gathering.pump();
    let host_seen = positions_seen(&gathering.host_events);
    assert_eq!(host_seen.len(), 15);
    for (index, (_, events)) in gathering.guests.iter().enumerate() {
        let own = u8::try_from(index + 1).expect("slot");
        let mut senders: Vec<u8> = positions_seen(events)
            .iter()
            .map(|(from, _)| *from)
            .collect();
        senders.sort_unstable();
        let expected: Vec<u8> = (1..16).filter(|slot| *slot != own).collect();
        assert_eq!(senders, expected);
    }
}

fn joined_slots(events: &[SessionEvent<Crowd>]) -> Vec<(u8, String)> {
    events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::PlayerJoined { slot, name, .. } => Some((*slot, name.clone())),
            _ => None,
        })
        .collect()
}

fn left_slots(events: &[SessionEvent<Crowd>]) -> Vec<u8> {
    events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::PlayerLeft { slot, .. } => Some(*slot),
            _ => None,
        })
        .collect()
}

#[test]
fn a_new_guest_is_told_of_the_guests_already_in() {
    let gathering = gathering_of(3);
    let (_, events) = &gathering.guests[2];
    assert_eq!(
        joined_slots(events),
        vec![
            (0, "Host".to_owned()),
            (1, "Guest0".to_owned()),
            (2, "Guest1".to_owned()),
        ]
    );
    let (_, first) = &gathering.guests[0];
    assert!(!joined_slots(first).iter().any(|(slot, _)| *slot == 1));
}

#[test]
fn the_guests_already_in_are_told_of_a_new_guest() {
    let gathering = gathering_of(3);
    assert_eq!(
        joined_slots(&gathering.guests[0].1),
        vec![
            (0, "Host".to_owned()),
            (2, "Guest1".to_owned()),
            (3, "Guest2".to_owned()),
        ]
    );
    assert_eq!(
        joined_slots(&gathering.guests[1].1),
        vec![
            (0, "Host".to_owned()),
            (1, "Guest0".to_owned()),
            (3, "Guest2".to_owned()),
        ]
    );
}

#[test]
fn a_leaving_guest_is_announced_to_the_rest() {
    let mut gathering = gathering_of(3);
    let (mut leaver, _) = gathering.guests.remove(1);
    leaver.close();
    drop(leaver);
    gathering.pump();
    assert_eq!(left_slots(&gathering.host_events), vec![2]);
    for (_, events) in &gathering.guests {
        assert_eq!(left_slots(events), vec![2]);
    }
}

#[test]
fn a_member_frame_from_a_guest_ends_that_guest() {
    let network = LoopbackNetwork::new();
    let mut host = pair_session(&network, Role::Host, "Host");
    let mut events = Vec::new();
    let (mut raw, host_id) = raw_guest_says_hello(&network, &mut host, 4, &mut events);
    let announcement = Pair::Member(Member {
        slot: 1,
        name: "Other".to_owned(),
    });
    let frame = encode_enveloped(&GUEST_ENVELOPE, &announcement).expect("encodes");
    raw.send(host_id, Channel::ReliableOrdered, &frame)
        .expect("sends");
    host.poll(Instant::now(), &mut events);
    assert!(events.iter().any(|event| matches!(
        event,
        SessionEvent::PlayerLeft {
            reason: LeaveReason::ProtocolViolation,
            ..
        }
    )));
    assert!(host.ready_peers().is_empty());
}

#[test]
fn a_frame_with_an_impossible_sender_is_dropped_by_a_guest() {
    let network = LoopbackNetwork::new();
    let mut raw_host = network.endpoint();
    let invite = raw_host.host().expect("invite");
    let mut guest = pair_session(&network, Role::Guest, "Guest");
    guest.join(&invite).expect("join");
    let mut guest_events = Vec::new();
    guest.poll(Instant::now(), &mut guest_events);
    let mut raw_events = Vec::new();
    raw_host.poll(&mut raw_events);
    let guest_id = raw_events
        .iter()
        .find_map(|event| match event {
            TransportEvent::PeerConnected(peer) => Some(*peer),
            _ => None,
        })
        .expect("guest connected");
    let welcome = Pair::Welcome(Welcome {
        version: 4,
        slot: 1,
        host_name: "Host".to_owned(),
        rules: rules_for(2),
    });
    raw_host
        .send(
            guest_id,
            Channel::ReliableOrdered,
            &encode(&welcome).expect("encodes"),
        )
        .expect("sends");
    guest.poll(Instant::now(), &mut guest_events);
    for from in [1, 2, 9, 0] {
        let envelope = Envelope { from, to: None };
        let frame =
            encode_enveloped(&envelope, &Pair::Chat(format!("from {from}"))).expect("encodes");
        raw_host
            .send(guest_id, Channel::ReliableOrdered, &frame)
            .expect("sends");
    }
    guest.poll(Instant::now(), &mut guest_events);
    assert_eq!(relayed_chats(&guest_events), vec![(0, "from 0".to_owned())]);
    assert_eq!(guest.ready_peers().len(), 1);
}

const ONE_SECOND: Duration = Duration::from_secs(1);

fn pump_for_seconds(gathering: &mut Gathering, start: Instant, seconds: u32) {
    for second in 1..=seconds {
        gathering.pump_at(start + ONE_SECOND * second);
    }
}

fn ping_lists(events: &[SessionEvent<Crowd>]) -> Vec<Vec<MemberPing>> {
    events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::MemberPings(pings) => Some(pings.clone()),
            _ => None,
        })
        .collect()
}

fn leave_reasons(events: &[SessionEvent<Crowd>]) -> Vec<(u8, LeaveReason)> {
    events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::PlayerLeft { slot, reason, .. } => Some((*slot, *reason)),
            _ => None,
        })
        .collect()
}

#[test]
fn every_guest_learns_every_members_ping() {
    let mut gathering = gathering_of(2);
    pump_for_seconds(&mut gathering, Instant::now(), 4);
    for (_, events) in &gathering.guests {
        let lists = ping_lists(events);
        let list = lists.last().expect("a ping list arrived");
        let slots: Vec<u8> = list.iter().map(|ping| ping.slot).collect();
        assert_eq!(slots, vec![0, 1, 2]);
        assert_eq!(list[0].rtt_ms, 0);
    }
    assert!(ping_lists(&gathering.host_events).is_empty());
}

#[test]
fn a_ping_list_from_a_guest_ends_that_guest() {
    let network = LoopbackNetwork::new();
    let mut host = pair_session(&network, Role::Host, "Host");
    let mut events = Vec::new();
    let (mut raw, host_id) = raw_guest_says_hello(&network, &mut host, 4, &mut events);
    let frame = encode_enveloped(&GUEST_ENVELOPE, &Pair::MemberPings(Vec::new())).expect("encodes");
    raw.send(host_id, Channel::ReliableOrdered, &frame)
        .expect("sends");
    host.poll(Instant::now(), &mut events);
    assert!(events.iter().any(|event| matches!(
        event,
        SessionEvent::PlayerLeft {
            reason: LeaveReason::ProtocolViolation,
            ..
        }
    )));
    assert!(host.ready_peers().is_empty());
}

#[test]
fn the_host_removes_a_guest_and_the_rest_are_told() {
    let mut gathering = gathering_of(3);
    assert!(gathering.host.remove(2));
    let start = Instant::now();
    pump_for_seconds(&mut gathering, start, 1);
    assert_eq!(
        leave_reasons(&gathering.host_events),
        vec![(2, LeaveReason::RemovedByHost)]
    );
    assert_eq!(left_slots(&gathering.guests[0].1), vec![2]);
    assert_eq!(left_slots(&gathering.guests[2].1), vec![2]);
    assert_eq!(gathering.host.ready_peers().len(), 2);
}

#[test]
fn a_removed_guest_learns_it_was_removed() {
    let mut gathering = gathering_of(2);
    assert!(gathering.host.remove(1));
    pump_for_seconds(&mut gathering, Instant::now(), 1);
    assert_eq!(
        leave_reasons(&gathering.guests[0].1),
        vec![(0, LeaveReason::RemovedByHost)]
    );
    assert!(
        leave_reasons(&gathering.guests[1].1)
            .iter()
            .all(|(slot, _)| *slot == 1)
    );
}

#[test]
fn a_removed_guest_may_join_again() {
    let mut gathering = Gathering::with_cap(MAX_SESSION_SLOTS);
    let invite = gathering.host.host().expect("invite");
    let mut guest = Session::new(
        gathering.network.endpoint(),
        config_for(Role::Guest, "Again", MAX_SESSION_SLOTS),
    );
    guest.join(&invite).expect("join");
    gathering.guests.push((guest, Vec::new()));
    gathering.pump();
    assert!(gathering.host.remove(1));
    pump_for_seconds(&mut gathering, Instant::now(), 1);
    assert!(gathering.host.ready_peers().is_empty());
    gathering.guests[0].1.clear();
    gathering.guests[0].0.join(&invite).expect("join again");
    gathering.pump_at(Instant::now() + ONE_SECOND * 2);
    assert_eq!(gathering.guests[0].0.local_slot(), Some(1));
    assert_eq!(gathering.host.ready_peers().len(), 1);
    assert!(joined_slots(&gathering.guests[0].1).contains(&(0, "Host".to_owned())));
}

#[test]
fn a_guest_cannot_remove_anyone() {
    let mut gathering = gathering_of(2);
    assert!(!gathering.guests[0].0.remove(2));
    assert!(!gathering.guests[0].0.remove(0));
    pump_for_seconds(&mut gathering, Instant::now(), 1);
    assert!(left_slots(&gathering.host_events).is_empty());
    assert_eq!(gathering.host.ready_peers().len(), 2);
}

#[test]
fn a_guest_session_reports_its_hello_and_the_welcome() {
    let network = LoopbackNetwork::new();
    let mut host: Session<LoopbackTransport, Current> =
        Session::new(network.endpoint(), config(Role::Host, "Host"));
    let mut guest: Session<LoopbackTransport, Current> =
        Session::new(network.endpoint(), config(Role::Guest, "Guest"));
    let board = ProgressBoard::default();
    guest.report_progress(board.fresh_sink());
    let invite = host.host().expect("invite");
    guest.join(&invite).expect("join");
    let (mut host_events, mut guest_events) = (Vec::new(), Vec::new());
    pump(&mut [
        (&mut host, &mut host_events),
        (&mut guest, &mut guest_events),
    ]);
    let told: Vec<LinkProgress> = board
        .entries()
        .into_iter()
        .map(|entry| entry.progress)
        .collect();
    assert_eq!(
        told,
        [
            LinkProgress::HelloSent { protocol: 4 },
            LinkProgress::WelcomeRead {
                seat: 1,
                host_name: "Host".to_owned(),
            },
        ]
    );
}
