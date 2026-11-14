use std::thread;
use std::time::{Duration, Instant};

use nc_hook::net::{
    LinkAccess, LinkPeer, NetGame, NetState, NetThread, PlayerOf, RemotePlayer, first_state_tick,
};
use nc_net::{
    Channel, LoopbackNetwork, LoopbackTransport, PeerId, Role, Session, SessionConfig,
    SessionEvent, Transport, TransportEvent,
};
use nc_wire::{
    BuildGate, Envelope, GameMessage, Hello, LinkPayload, MAX_SESSION_SLOTS, Member, MemberPing,
    Ping, Pong, Reject, STORY_MODE, SessionRules, Welcome, WireError, decode_enveloped,
    encode_enveloped,
};
use serde::{Deserialize, Serialize};

const TWO_PLAYER_RULES: SessionRules = SessionRules {
    mode: STORY_MODE,
    max_players: 2,
};
const WAIT_LIMIT: Duration = Duration::from_secs(20);
const MADE_UP_PROTOCOL_VERSION: u16 = 7;
const THREE_PLAYER_RULES: SessionRules = SessionRules {
    mode: STORY_MODE,
    max_players: 3,
};
const C_SLOT: u8 = 2;
const C_GUARD_NAME: &str = "Wren";
const WAVE_FROM_THE_SECOND_SENDER: i32 = 4;
const A_SLOT: u8 = 1;
const B_SLOT: u8 = 0;
const A_GUARD_NAME: &str = "Mira";
const B_GUARD_NAME: &str = "Odo";
const A_LANTERN_X: i32 = 5;
const B_LANTERN_X: i32 = 9;
const LANTERN_X_OF_A_COUNT_THAT_STARTS_AT_ONE: i32 = 77;
const PAUSE_BEFORE_THE_WORKER_STARTS_AGAIN: Duration = Duration::from_millis(100);
const SESSION_PUMP_ROUNDS: usize = 6;
const WAVE_SENT_WHILE_NOBODY_IS_IN_PLAY: i32 = 1;
const WAVE_SENT_ONCE_THE_PARTNER_IS_IN_PLAY: i32 = 2;
const WAVE_FROM_THE_PARTNER: i32 = 3;
const FIRST_FLUSH_NONCE: u32 = 41;
const SECOND_FLUSH_NONCE: u32 = 42;
const MADE_UP_BUILD: BuildGate = BuildGate {
    time_date_stamp: 7,
    size_of_image: 9,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum LanternMessage {
    Hello(Hello),
    Welcome(Welcome),
    Reject(Reject),
    Ping(Ping),
    Pong(Pong),
    Bye,
    Lantern { tick: u32, x: i32 },
    Member(Member),
    MemberLeft { slot: u8 },
    Rules(SessionRules),
    MemberPings(Vec<MemberPing>),
    Wave { x: i32 },
    InPlay,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct LanternWatch {
    in_play: bool,
}

impl GameMessage for LanternMessage {
    const PROTOCOL_VERSION: u16 = MADE_UP_PROTOCOL_VERSION;
    const MAX_PLAYERS: u8 = MAX_SESSION_SLOTS;

    fn from_link(payload: LinkPayload) -> Self {
        match payload {
            LinkPayload::Hello(hello) => Self::Hello(hello),
            LinkPayload::Welcome(welcome) => Self::Welcome(welcome),
            LinkPayload::Reject(reject) => Self::Reject(reject),
            LinkPayload::Ping(ping) => Self::Ping(ping),
            LinkPayload::Pong(pong) => Self::Pong(pong),
            LinkPayload::Bye => Self::Bye,
            LinkPayload::Member(member) => Self::Member(member),
            LinkPayload::MemberLeft { slot } => Self::MemberLeft { slot },
            LinkPayload::Rules(rules) => Self::Rules(rules),
            LinkPayload::MemberPings(pings) => Self::MemberPings(pings),
        }
    }

    fn into_link(self) -> Result<LinkPayload, Self> {
        match self {
            Self::Hello(hello) => Ok(LinkPayload::Hello(hello)),
            Self::Welcome(welcome) => Ok(LinkPayload::Welcome(welcome)),
            Self::Reject(reject) => Ok(LinkPayload::Reject(reject)),
            Self::Ping(ping) => Ok(LinkPayload::Ping(ping)),
            Self::Pong(pong) => Ok(LinkPayload::Pong(pong)),
            Self::Bye => Ok(LinkPayload::Bye),
            Self::Member(member) => Ok(LinkPayload::Member(member)),
            Self::MemberLeft { slot } => Ok(LinkPayload::MemberLeft { slot }),
            Self::Rules(rules) => Ok(LinkPayload::Rules(rules)),
            Self::MemberPings(pings) => Ok(LinkPayload::MemberPings(pings)),
            other @ (Self::Lantern { .. } | Self::Wave { .. } | Self::InPlay) => Err(other),
        }
    }

    fn is_relayed(&self) -> bool {
        matches!(self, Self::Wave { .. })
    }

    fn is_latest_wins(&self) -> bool {
        matches!(self, Self::Lantern { .. })
    }

    fn ordered_tick(&self) -> Option<u32> {
        match self {
            Self::Lantern { tick, .. } => Some(*tick),
            _ => None,
        }
    }

    fn validate(&self) -> Result<(), WireError> {
        Ok(())
    }
}

struct LanternGame;

impl NetGame for LanternGame {
    type Message = LanternMessage;
    type Snapshot = i32;
    type PeerState = i32;
    type PlayerWatch = LanternWatch;
    type Extras = ();

    fn kind_name(message: &LanternMessage) -> &'static str {
        match message {
            LanternMessage::Lantern { .. } => "Lantern",
            LanternMessage::Wave { .. } => "Wave",
            _ => "Link",
        }
    }

    fn player_in_play(watch: &LanternWatch) -> bool {
        watch.in_play
    }

    fn snapshot_message(&mut self, snapshot: &i32, tick: u32) -> LanternMessage {
        LanternMessage::Lantern { tick, x: *snapshot }
    }

    fn on_message(&mut self, link: &LinkAccess<'_, Self>, from: u8, message: LanternMessage) {
        match message {
            LanternMessage::Lantern { x, .. } => {
                if let Some(player) = link.shared().players.get_mut(&from) {
                    player.latest = Some(RemotePlayer {
                        name: player.name.clone(),
                        slot: from,
                        state: x,
                        received_at: Instant::now(),
                    });
                }
            }
            LanternMessage::InPlay => {
                if let Some(player) = link.shared().players.get_mut(&from) {
                    player.watch.in_play = true;
                }
            }
            wave @ LanternMessage::Wave { .. } => link.push_relayed(from, wave),
            _ => {}
        }
    }

    fn on_player_joined(&mut self, _extras: &mut (), _player: &mut PlayerOf<Self>) {}

    fn on_peer_cleared(&mut self, _extras: &mut ()) {}

    fn on_launcher_linked(&mut self) {}

    fn on_idle(
        &mut self,
        _link: &LinkAccess<'_, Self>,
        _send: &mut dyn FnMut(&LanternMessage, Channel) -> bool,
    ) {
    }

    fn announce(&self, _text: &str) {}
}

struct LauncherPair {
    host: Session<LoopbackTransport, LanternMessage>,
    guest: Session<LoopbackTransport, LanternMessage>,
    host_events: Vec<SessionEvent<LanternMessage>>,
}

impl LauncherPair {
    fn connected() -> Self {
        let network = LoopbackNetwork::new();
        let mut host = Session::new(
            network.endpoint(),
            SessionConfig {
                role: Role::Host,
                name: B_GUARD_NAME.to_owned(),
                build: MADE_UP_BUILD,
                rules: TWO_PLAYER_RULES,
            },
        );
        let mut guest = Session::new(
            network.endpoint(),
            SessionConfig {
                role: Role::Guest,
                name: A_GUARD_NAME.to_owned(),
                build: MADE_UP_BUILD,
                rules: TWO_PLAYER_RULES,
            },
        );
        let invite = host.host().expect("invite");
        guest.join(&invite).expect("join");
        let mut pair = Self {
            host,
            guest,
            host_events: Vec::new(),
        };
        pair.pump();
        pair
    }

    fn pump(&mut self) {
        let now = Instant::now();
        let mut guest_events = Vec::new();
        for _ in 0..SESSION_PUMP_ROUNDS {
            self.host.poll(now, &mut self.host_events);
            self.guest.poll(now, &mut guest_events);
        }
    }

    fn forward_from_the_guest(&mut self, message: &LanternMessage) {
        self.guest.send_ordered(message).expect("an ordered kind");
        self.pump();
    }

    fn lanterns_the_host_took(&self) -> Vec<i32> {
        self.host_events
            .iter()
            .filter_map(|event| match event {
                SessionEvent::Message {
                    message: LanternMessage::Lantern { x, .. },
                    ..
                } => Some(*x),
                _ => None,
            })
            .collect()
    }
}

struct LinkedWorker {
    worker: NetThread<LanternGame>,
    launcher_end: LoopbackTransport,
    worker_peer: PeerId,
}

impl LinkedWorker {
    fn linked() -> Self {
        let network = LoopbackNetwork::new();
        let worker_transport = network.endpoint();
        let mut launcher_end = network.endpoint();
        let worker_peer = worker_transport.local_peer();
        let invite = launcher_end.host().expect("the launcher end hosts");
        let worker = NetThread::spawn(
            LanternGame,
            worker_transport,
            LinkPeer {
                invite: Some(invite),
                peer: launcher_end.local_peer(),
                port: 1,
            },
        )
        .expect("the worker starts");
        assert!(wait_until(
            || worker.view().state == NetState::LauncherLinked
        ));
        Self {
            worker,
            launcher_end,
            worker_peer,
        }
    }

    fn welcomed_with(rules: SessionRules) -> Self {
        let mut linked = Self::linked();
        linked.send(B_SLOT, &welcome(A_SLOT, B_GUARD_NAME, rules));
        assert!(wait_until(
            || linked.worker.view().local_slot == Some(A_SLOT)
        ));
        linked
    }

    fn with_players(rules: SessionRules, others: &[(u8, &str)]) -> Self {
        let mut linked = Self::welcomed_with(rules);
        for (slot, name) in others {
            linked.send(B_SLOT, &member(*slot, name));
        }
        assert!(wait_until(
            || linked.worker.view().players.len() == others.len()
        ));
        assert!(wait_until(
            || linked.worker.view().state == NetState::Connected
        ));
        linked
    }

    fn with_partner() -> Self {
        Self::with_players(TWO_PLAYER_RULES, &[(B_SLOT, B_GUARD_NAME)])
    }

    fn send(&mut self, from: u8, message: &LanternMessage) {
        let frame =
            encode_enveloped(&Envelope { from, to: None }, message).expect("a frame encodes");
        self.launcher_end
            .send(self.worker_peer, Channel::ReliableOrdered, &frame)
            .expect("the frame is sent");
    }

    fn received(&mut self) -> Vec<(Envelope, LanternMessage)> {
        let mut events = Vec::new();
        self.launcher_end.poll(&mut events);
        events
            .into_iter()
            .filter_map(|event| match event {
                TransportEvent::Message { payload, .. } => {
                    decode_enveloped::<LanternMessage>(&payload).ok()
                }
                _ => None,
            })
            .collect()
    }

    fn waves_until_the_worker_is_flushed(&mut self) -> Vec<i32> {
        // Two rounds of the worker loop pass before the second ping, so every relay queued earlier is sent or dropped.
        let mut waves = Vec::new();
        for nonce in [FIRST_FLUSH_NONCE, SECOND_FLUSH_NONCE] {
            self.worker
                .send_to_launcher(LanternMessage::Ping(Ping { nonce }));
            let mut flushed = false;
            assert!(wait_until(|| {
                for (_, message) in self.received() {
                    match message {
                        LanternMessage::Wave { x } => waves.push(x),
                        LanternMessage::Ping(ping) if ping.nonce == nonce => flushed = true,
                        _ => {}
                    }
                }
                flushed
            }));
        }
        waves
    }
}

fn wait_until(mut condition: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + WAIT_LIMIT;
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        thread::sleep(Duration::from_millis(2));
    }
    false
}

fn welcome(slot: u8, host_name: &str, rules: SessionRules) -> LanternMessage {
    LanternMessage::Welcome(Welcome {
        version: MADE_UP_PROTOCOL_VERSION,
        slot,
        host_name: host_name.to_owned(),
        rules,
    })
}

fn member(slot: u8, name: &str) -> LanternMessage {
    LanternMessage::Member(Member {
        slot,
        name: name.to_owned(),
    })
}

fn is_wave(message: &LanternMessage) -> bool {
    matches!(message, LanternMessage::Wave { .. })
}

fn start_pair() -> (NetThread<LanternGame>, NetThread<LanternGame>) {
    let network = LoopbackNetwork::new();
    let a_transport = network.endpoint();
    let mut b_transport = network.endpoint();
    let a_peer = a_transport.local_peer();
    let b_peer = b_transport.local_peer();
    let b_invite = b_transport.host().expect("b hosts");
    let a = NetThread::spawn(
        LanternGame,
        a_transport,
        LinkPeer {
            invite: Some(b_invite),
            peer: b_peer,
            port: 1,
        },
    )
    .expect("a starts");
    let b = NetThread::spawn(
        LanternGame,
        b_transport,
        LinkPeer {
            invite: None,
            peer: a_peer,
            port: 2,
        },
    )
    .expect("b starts");
    (a, b)
}

fn first_lantern_sent_by(linked: &mut LinkedWorker, x: i32) -> LanternMessage {
    let mut sent = None;
    assert!(wait_until(|| {
        linked.worker.publish(x);
        sent = linked
            .received()
            .into_iter()
            .map(|(_, message)| message)
            .find(|message| matches!(message, LanternMessage::Lantern { .. }));
        sent.is_some()
    }));
    sent.expect("the wait saw a lantern")
}

#[test]
fn the_states_of_a_worker_started_again_are_not_dropped_as_old() {
    let mut launchers = LauncherPair::connected();
    let mut first_run = LinkedWorker::with_partner();
    let from_the_first_run = first_lantern_sent_by(&mut first_run, A_LANTERN_X);
    drop(first_run);
    thread::sleep(PAUSE_BEFORE_THE_WORKER_STARTS_AGAIN);
    let mut second_run = LinkedWorker::with_partner();
    let from_the_second_run = first_lantern_sent_by(&mut second_run, B_LANTERN_X);
    let from_a_count_that_starts_at_one = LanternMessage::Lantern {
        tick: 1,
        x: LANTERN_X_OF_A_COUNT_THAT_STARTS_AT_ONE,
    };

    launchers.forward_from_the_guest(&from_the_first_run);
    launchers.forward_from_the_guest(&from_a_count_that_starts_at_one);
    launchers.forward_from_the_guest(&from_the_second_run);

    assert_eq!(
        launchers.lanterns_the_host_took(),
        vec![A_LANTERN_X, B_LANTERN_X]
    );
}

#[test]
fn the_state_count_starts_later_for_every_later_start() {
    let first_start = Duration::from_secs(1_700_000_000);
    let started_again = first_start + PAUSE_BEFORE_THE_WORKER_STARTS_AGAIN;
    let newer_by = first_state_tick(started_again).wrapping_sub(first_state_tick(first_start));
    assert!(newer_by as i32 > 0);
}

#[test]
fn the_state_count_wraps_instead_of_stopping_at_the_largest_number() {
    let step = Duration::from_millis(16);
    let at_the_largest = step * u32::MAX;
    assert_eq!(first_state_tick(at_the_largest), u32::MAX);
    assert_eq!(first_state_tick(at_the_largest + step), 0);
}

#[test]
fn two_workers_link_and_each_receives_the_others_message() {
    let (a, b) = start_pair();
    assert!(wait_until(|| {
        a.view().state == NetState::LauncherLinked && b.view().state == NetState::LauncherLinked
    }));

    a.send_to_launcher(welcome(B_SLOT, A_GUARD_NAME, TWO_PLAYER_RULES));
    a.send_to_launcher(member(A_SLOT, A_GUARD_NAME));
    b.send_to_launcher(welcome(A_SLOT, B_GUARD_NAME, TWO_PLAYER_RULES));
    b.send_to_launcher(member(B_SLOT, B_GUARD_NAME));
    assert!(wait_until(|| {
        a.view().partner().is_some() && b.view().partner().is_some()
    }));
    assert_eq!(a.view().state.label(), "connected");
    assert_eq!(b.view().state.label(), "connected");
    assert_eq!(a.view().partner_name(), Some(B_GUARD_NAME));
    assert_eq!(b.view().partner_name(), Some(A_GUARD_NAME));

    assert!(wait_until(|| {
        a.publish(A_LANTERN_X);
        b.publish(B_LANTERN_X);
        a.view()
            .partner_state()
            .is_some_and(|remote| remote.state == B_LANTERN_X)
            && b.view()
                .partner_state()
                .is_some_and(|remote| remote.state == A_LANTERN_X)
    }));
}

#[test]
fn a_worker_waits_for_the_link_before_it_is_connected() {
    let (a, b) = start_pair();
    assert!(wait_until(|| b.view().state == NetState::LauncherLinked));
    assert_eq!(b.view().state.label(), "launcher linked, no other player");
    assert!(!a.wants_snapshots());
    assert!(!b.wants_snapshots());
}

#[test]
fn members_fill_the_players_map_and_leave_it() {
    let mut linked = LinkedWorker::with_partner();
    let view = linked.worker.view();
    assert_eq!(
        view.players.keys().copied().collect::<Vec<_>>(),
        vec![B_SLOT]
    );
    assert_eq!(view.partner_name(), Some(B_GUARD_NAME));
    assert_eq!(view.local_slot, Some(A_SLOT));

    linked.send(B_SLOT, &LanternMessage::MemberLeft { slot: B_SLOT });
    assert!(wait_until(|| linked.worker.view().players.is_empty()));
    let view = linked.worker.view();
    assert_eq!(view.partner_name(), None);
    assert_eq!(view.state, NetState::LauncherLinked);
    assert_eq!(view.local_slot, Some(A_SLOT));
}

#[test]
fn partner_is_the_one_other_player() {
    let mut linked = LinkedWorker::with_partner();
    linked.send(B_SLOT, &member(A_SLOT, A_GUARD_NAME));
    linked.send(
        B_SLOT,
        &LanternMessage::Lantern {
            tick: 1,
            x: B_LANTERN_X,
        },
    );
    assert!(wait_until(|| linked
        .worker
        .view()
        .partner_state()
        .is_some()));

    let view = linked.worker.view();
    let partner = view.partner().expect("the partner");
    assert_eq!(partner.slot, B_SLOT);
    assert_eq!(
        view.partner_state().map(|state| state.state),
        Some(B_LANTERN_X)
    );
    assert_eq!(view.others().count(), 1);
    assert!(view.player(B_SLOT).is_some());
    assert!(view.player(A_SLOT).is_none());
}

#[test]
fn relayed_messages_keep_their_sender() {
    let mut linked = LinkedWorker::with_partner();
    linked.send(
        B_SLOT,
        &LanternMessage::Wave {
            x: WAVE_FROM_THE_PARTNER,
        },
    );
    let mut taken = Vec::new();
    assert!(wait_until(|| {
        taken.extend(linked.worker.take_relayed_from(is_wave));
        !taken.is_empty()
    }));
    assert_eq!(
        taken,
        vec![(
            B_SLOT,
            LanternMessage::Wave {
                x: WAVE_FROM_THE_PARTNER
            }
        )]
    );
}

#[test]
fn nothing_is_sent_while_no_player_is_in_play() {
    let mut linked = LinkedWorker::with_partner();
    linked.worker.send_relayed(LanternMessage::Wave {
        x: WAVE_SENT_WHILE_NOBODY_IS_IN_PLAY,
    });
    assert_eq!(
        linked.waves_until_the_worker_is_flushed(),
        Vec::<i32>::new()
    );

    linked.send(B_SLOT, &LanternMessage::InPlay);
    assert!(wait_until(|| {
        linked
            .worker
            .view()
            .player(B_SLOT)
            .is_some_and(|player| player.watch.in_play)
    }));
    linked.worker.send_relayed(LanternMessage::Wave {
        x: WAVE_SENT_ONCE_THE_PARTNER_IS_IN_PLAY,
    });
    assert_eq!(
        linked.waves_until_the_worker_is_flushed(),
        vec![WAVE_SENT_ONCE_THE_PARTNER_IS_IN_PLAY]
    );
}

#[test]
fn the_rules_of_the_welcome_are_kept_and_changed_by_rules() {
    let mut linked = LinkedWorker::welcomed_with(TWO_PLAYER_RULES);
    assert_eq!(linked.worker.view().rules, TWO_PLAYER_RULES);
    let other = SessionRules {
        mode: 3,
        max_players: 2,
    };
    linked.send(B_SLOT, &LanternMessage::Rules(other));
    assert!(wait_until(|| linked.worker.view().rules == other));
}

#[test]
fn a_worker_is_not_connected_until_another_player_is_in_the_session() {
    let mut linked = LinkedWorker::welcomed_with(THREE_PLAYER_RULES);
    assert_eq!(linked.worker.view().state, NetState::LauncherLinked);
    assert!(!linked.worker.wants_snapshots());
    linked.send(B_SLOT, &member(B_SLOT, B_GUARD_NAME));
    assert!(wait_until(
        || linked.worker.view().state == NetState::Connected
    ));
    assert!(linked.worker.wants_snapshots());
}

#[test]
fn three_players_are_kept_by_slot() {
    let linked = LinkedWorker::with_players(
        THREE_PLAYER_RULES,
        &[(C_SLOT, C_GUARD_NAME), (B_SLOT, B_GUARD_NAME)],
    );
    let view = linked.worker.view();
    assert_eq!(
        view.others().map(|player| player.slot).collect::<Vec<_>>(),
        vec![B_SLOT, C_SLOT]
    );
    assert_eq!(
        view.player(B_SLOT).map(|player| player.name.as_str()),
        Some(B_GUARD_NAME)
    );
    assert_eq!(
        view.player(C_SLOT).map(|player| player.name.as_str()),
        Some(C_GUARD_NAME)
    );
    assert!(view.player(A_SLOT).is_none());
}

#[test]
fn partner_is_none_when_the_session_allows_more_than_two() {
    let linked = LinkedWorker::with_players(THREE_PLAYER_RULES, &[(B_SLOT, B_GUARD_NAME)]);
    let view = linked.worker.view();
    assert_eq!(view.others().count(), 1);
    assert!(view.partner().is_none());
    assert!(view.partner_state().is_none());
    assert_eq!(view.partner_name(), None);
}

#[test]
fn a_two_player_session_has_a_partner() {
    let linked = LinkedWorker::with_partner();
    let view = linked.worker.view();
    assert_eq!(view.partner().map(|player| player.slot), Some(B_SLOT));
    assert_eq!(view.partner_name(), Some(B_GUARD_NAME));
}

#[test]
fn a_leaving_player_takes_only_its_own_state() {
    let mut linked = LinkedWorker::with_players(
        THREE_PLAYER_RULES,
        &[(B_SLOT, B_GUARD_NAME), (C_SLOT, C_GUARD_NAME)],
    );
    linked.send(
        B_SLOT,
        &LanternMessage::Lantern {
            tick: 1,
            x: B_LANTERN_X,
        },
    );
    linked.send(
        C_SLOT,
        &LanternMessage::Lantern {
            tick: 1,
            x: A_LANTERN_X,
        },
    );
    assert!(wait_until(|| {
        let view = linked.worker.view();
        view.others().all(|player| player.latest.is_some())
    }));

    linked.send(B_SLOT, &LanternMessage::MemberLeft { slot: B_SLOT });
    assert!(wait_until(|| linked.worker.view().player(B_SLOT).is_none()));
    let view = linked.worker.view();
    assert_eq!(view.state, NetState::Connected);
    assert_eq!(
        view.player(C_SLOT)
            .and_then(|player| player.latest.as_ref())
            .map(|state| state.state),
        Some(A_LANTERN_X)
    );

    linked.send(B_SLOT, &LanternMessage::MemberLeft { slot: C_SLOT });
    assert!(wait_until(|| linked.worker.view().players.is_empty()));
    assert_eq!(linked.worker.view().state, NetState::LauncherLinked);
}

#[test]
fn relayed_messages_of_two_senders_stay_apart() {
    let mut linked = LinkedWorker::with_players(
        THREE_PLAYER_RULES,
        &[(B_SLOT, B_GUARD_NAME), (C_SLOT, C_GUARD_NAME)],
    );
    linked.send(
        B_SLOT,
        &LanternMessage::Wave {
            x: WAVE_FROM_THE_PARTNER,
        },
    );
    linked.send(
        C_SLOT,
        &LanternMessage::Wave {
            x: WAVE_FROM_THE_SECOND_SENDER,
        },
    );
    let mut taken = Vec::new();
    assert!(wait_until(|| {
        taken.extend(linked.worker.take_relayed_from(is_wave));
        taken.len() == 2
    }));
    taken.sort_by_key(|(slot, _)| *slot);
    assert_eq!(
        taken,
        vec![
            (
                B_SLOT,
                LanternMessage::Wave {
                    x: WAVE_FROM_THE_PARTNER
                }
            ),
            (
                C_SLOT,
                LanternMessage::Wave {
                    x: WAVE_FROM_THE_SECOND_SENDER
                }
            ),
        ]
    );
}
