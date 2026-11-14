use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use nc_launcher_core::{
    LauncherConfig, LauncherCore, LauncherEvent, LauncherNet, LinkStatus, MemberChange, Members,
    NetState, PathKind, TransportFactory,
};
use nc_net::{
    Channel, Invite, LINK_INVITE_PREFIX, LINK_LAUNCHER_PEER, LocalLinkTransport, LoopbackNetwork,
    LoopbackTransport, PeerId, Role, Session, SessionConfig, SessionEvent, Transport,
    TransportEvent,
};
use nc_wire::{Envelope, STORY_MODE, SessionRules, decode_enveloped, encode_enveloped};
use replicant_proto::{
    CampaignCard, EnemyGone, EnemyGoneReason, EnemyId, GameReadiness, LOBBY_MODE_NEW_GAME,
    LobbyReady, Message, PartnerBody, PlayerSettings, PlayerState, SaveSummary, SeatOutcome,
    SeatReceipt, SeatRefusal, SecondSeatCard, SessionOptions,
};

use crate::seat_assembler::{all_parts, generated_seat};

const WAIT_LIMIT: Duration = Duration::from_secs(20);
const CHANGED_SETTINGS: PlayerSettings = PlayerSettings {
    keep_running: false,
    event_log: false,
};

const OWN_GAME_ENVELOPE: Envelope = Envelope { from: 0, to: None };

const GUEST_READY_NEW_GAME: LobbyReady = LobbyReady {
    ready: true,
    mode: LOBBY_MODE_NEW_GAME,
    slot: None,
};

fn partner_lobby(core: &LauncherCore) -> Option<LobbyReady> {
    core.link_status()
        .partner()
        .and_then(|partner| partner.lobby)
}

fn partner_body(core: &LauncherCore) -> Option<PartnerBody> {
    core.link_status()
        .partner()
        .and_then(|partner| partner.body)
}

pub struct FakeGame {
    link: LocalLinkTransport,
    pub received: Vec<Message>,
    pub envelopes: Vec<Envelope>,
}

impl FakeGame {
    pub fn attach(core: &LauncherCore, config: &LauncherConfig) -> Self {
        let mut link = LocalLinkTransport::game(config.build);
        let invite =
            Invite::new(format!("{LINK_INVITE_PREFIX}{}", core.link_port())).expect("invite");
        link.join(&invite).expect("dials");
        Self {
            link,
            received: Vec::new(),
            envelopes: Vec::new(),
        }
    }

    pub fn pump(&mut self) {
        let mut events = Vec::new();
        self.link.poll(&mut events);
        for event in events {
            if let TransportEvent::Message { payload, .. } = event
                && let Ok((envelope, message)) = decode_enveloped(&payload)
            {
                self.envelopes.push(envelope);
                self.received.push(message);
            }
        }
    }

    pub fn send_state(&mut self, state: &PlayerState) {
        self.send_on(Channel::UnreliableLatest, &Message::State(*state));
    }

    pub fn send_readiness(&mut self, readiness: GameReadiness) {
        self.send_message(&Message::Readiness(readiness));
    }

    pub fn send_message(&mut self, message: &Message) {
        self.send_on(Channel::ReliableOrdered, message);
    }

    pub fn send_on(&mut self, channel: Channel, message: &Message) {
        let bytes = encode_enveloped(&OWN_GAME_ENVELOPE, message).expect("encodes");
        let _ = self.link.send(LINK_LAUNCHER_PEER, channel, &bytes);
    }

    pub fn sender_of(&mut self, matcher: impl Fn(&Message) -> bool) -> Option<u8> {
        self.pump();
        let index = self.received.iter().position(matcher)?;
        Some(self.envelopes[index].from)
    }

    pub fn saw(&mut self, matcher: impl Fn(&Message) -> bool) -> bool {
        self.pump();
        self.received.iter().any(matcher)
    }
}

pub fn wait_until(mut condition: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + WAIT_LIMIT;
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        thread::sleep(Duration::from_millis(3));
    }
    false
}

pub fn state_at(tick: u32, x: f32) -> PlayerState {
    PlayerState {
        tick,
        phase: 2,
        position: [x, 0.0, 0.0],
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

pub fn loopback_factory(network: &LoopbackNetwork) -> TransportFactory {
    let network = network.clone();
    Arc::new(move || Ok(Box::new(network.endpoint()) as Box<dyn Transport>))
}

pub struct Rig {
    pub host: LauncherCore,
    pub guest: LauncherCore,
    pub host_game: FakeGame,
    pub guest_game: FakeGame,
}

pub fn rig() -> Rig {
    let network = LoopbackNetwork::new();
    let host = LauncherCore::new(LauncherConfig::new("Hosty"), loopback_factory(&network))
        .expect("host core");
    let guest = LauncherCore::new(LauncherConfig::new("Guesty"), loopback_factory(&network))
        .expect("guest core");
    let host_game = FakeGame::attach(&host, &LauncherConfig::new("x"));
    let guest_game = FakeGame::attach(&guest, &LauncherConfig::new("x"));
    Rig {
        host,
        guest,
        host_game,
        guest_game,
    }
}

pub fn connect(rig: &mut Rig) {
    let ticket = rig.host.host().expect("ticket");
    rig.guest.join(&ticket).expect("join starts");
}

#[test]
fn hosting_returns_a_ticket_and_reports_it_in_the_status() {
    let rig = rig();
    let ticket = rig.host.host().expect("ticket");
    let status = rig.host.status();
    assert!(matches!(&status.state, NetState::Hosting { ticket: t } if *t == ticket));
}

#[test]
fn both_games_get_a_welcome_with_the_other_players_name() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| rig.host_game.saw(|m| matches!(
        m,
        Message::Welcome(w) if w.host_name == "Guesty" && w.slot == 0
    ))));
    assert!(wait_until(|| rig.guest_game.saw(|m| matches!(
        m,
        Message::Welcome(w) if w.host_name == "Hosty" && w.slot == 1
    ))));
    assert!(matches!(
        rig.guest.status().state,
        NetState::Connected { .. }
    ));
    assert!(rig.host.link_status().game_linked);
}

#[test]
fn a_two_player_session_still_announces_one_partner() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| rig.host_game.saw(|m| matches!(
        m,
        Message::Member(member) if member.slot == 1 && member.name == "Guesty"
    ))));
    assert!(wait_until(|| rig.guest_game.saw(|m| matches!(
        m,
        Message::Member(member) if member.slot == 0 && member.name == "Hosty"
    ))));
    thread::sleep(Duration::from_millis(200));
    for game in [&mut rig.host_game, &mut rig.guest_game] {
        game.pump();
        let members = game
            .received
            .iter()
            .filter(|message| matches!(message, Message::Member(_)))
            .count();
        assert_eq!(members, 1);
        assert!(
            !game
                .received
                .iter()
                .any(|message| matches!(message, Message::MemberLeft { .. }))
        );
    }
}

#[test]
fn player_state_crosses_both_links_and_the_session() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| rig
        .host_game
        .saw(|m| matches!(m, Message::Welcome(_)))));
    assert!(wait_until(|| rig
        .guest_game
        .saw(|m| matches!(m, Message::Welcome(_)))));
    let mut tick = 0;
    assert!(wait_until(|| {
        tick += 1;
        rig.host_game.send_state(&state_at(tick, 5.0));
        rig.guest_game
            .saw(|m| matches!(m, Message::State(s) if s.position[0] == 5.0 && s.phase == 2))
    }));
    assert!(wait_until(|| {
        tick += 1;
        rig.guest_game.send_state(&state_at(tick, -3.0));
        rig.host_game
            .saw(|m| matches!(m, Message::State(s) if s.position[0] == -3.0))
    }));
}

#[test]
fn an_enemy_event_from_the_host_game_reaches_the_guest_game() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| rig
        .guest_game
        .saw(|m| matches!(m, Message::Welcome(_)))));
    let gone = Message::EnemyGone(EnemyGone {
        phase: 2,
        id: EnemyId {
            group: 40,
            ordinal: 1,
        },
        reason: EnemyGoneReason::Removed,
    });
    assert!(wait_until(|| {
        rig.host_game.send_message(&gone);
        rig.guest_game.saw(|m| *m == gone)
    }));
}

#[test]
fn path_info_reaches_the_game() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| rig
        .guest_game
        .saw(|m| matches!(m, Message::PathInfo(info) if info.direct))));
}

#[test]
fn a_guest_with_the_ticket_connects() {
    let rig = rig();
    let ticket = rig.host.host().expect("ticket");
    rig.guest.join(&ticket).expect("join");
    assert!(wait_until(|| matches!(
        rig.guest.status().state,
        NetState::Connected { .. }
    )));
}

#[test]
fn the_host_stopping_tells_the_guest_game_goodbye() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| rig
        .guest_game
        .saw(|m| matches!(m, Message::Welcome(_)))));
    rig.host.leave();
    assert!(wait_until(|| rig
        .guest_game
        .saw(|m| matches!(m, Message::Bye))));
    assert_eq!(rig.host.status().state, NetState::Idle);
}

#[test]
fn a_game_that_attaches_late_is_welcomed_when_the_peer_is_already_here() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| matches!(
        rig.guest.status().state,
        NetState::Connected { .. }
    )));
    let late = FakeGame::attach(&rig.guest, &LauncherConfig::new("x"));
    drop(std::mem::replace(&mut rig.guest_game, late));
    assert!(wait_until(|| rig.guest_game.saw(
        |m| matches!(m, Message::Welcome(w) if w.host_name == "Hosty")
    )));
}

#[test]
fn a_game_on_another_build_cannot_attach() {
    let network = LoopbackNetwork::new();
    let core =
        LauncherCore::new(LauncherConfig::new("Hosty"), loopback_factory(&network)).expect("core");
    let mut other = LauncherConfig::new("x");
    other.build.size_of_image ^= 1;
    let _game = FakeGame::attach(&core, &other);
    thread::sleep(Duration::from_millis(700));
    assert!(!core.link_status().game_linked);
}

#[test]
fn test_connection_reports_a_good_loopback_link() {
    let rig = rig();
    let ticket = rig.host.host().expect("ticket");
    let report = rig.guest.test_connection(&ticket, 2).expect("probe runs");
    assert!(report.reachable);
    assert_eq!(report.path, PathKind::Direct);
    assert!(report.rtt.is_some());
    assert!(report.verdict.contains("good"));
}

#[test]
fn test_connection_to_nobody_reports_unreachable() {
    let network = LoopbackNetwork::new();
    let core =
        LauncherCore::new(LauncherConfig::new("Guesty"), loopback_factory(&network)).expect("core");
    let report = core
        .test_connection("loopback-999", 2)
        .map(|report| report.reachable);
    assert!(matches!(report, Ok(false) | Err(_)));
}

#[test]
fn a_host_game_that_is_still_loading_is_reported_to_the_guest() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| rig
        .guest_game
        .saw(|m| *m == Message::Readiness(GameReadiness::Loading))));
    assert_eq!(
        rig.guest.link_status().partner_readiness(),
        GameReadiness::Loading
    );
}

#[test]
fn a_host_game_going_in_play_is_reported_to_the_guest_and_its_game() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| {
        rig.host_game.send_readiness(GameReadiness::InPlay);
        rig.guest_game
            .saw(|m| *m == Message::Readiness(GameReadiness::InPlay))
    }));
    assert_eq!(
        rig.guest.link_status().partner_readiness(),
        GameReadiness::InPlay
    );
}

#[test]
fn a_host_game_that_closes_is_reported_not_linked_and_a_new_one_starts_loading() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| {
        rig.host_game.send_readiness(GameReadiness::InPlay);
        rig.guest.link_status().partner_readiness() == GameReadiness::InPlay
    }));
    let Rig {
        host,
        guest,
        host_game,
        ..
    } = rig;
    drop(host_game);
    assert!(wait_until(
        || guest.link_status().partner_readiness() == GameReadiness::NotLinked
    ));
    let _returned = FakeGame::attach(&host, &LauncherConfig::new("x"));
    assert!(wait_until(
        || guest.link_status().partner_readiness() == GameReadiness::Loading
    ));
}

#[test]
fn a_game_that_reports_before_a_guest_joins_is_still_known_when_the_guest_arrives() {
    let mut rig = rig();
    let ticket = rig.host.host().expect("ticket");
    assert!(wait_until(|| rig.host.link_status().game_linked));
    for _ in 0..15 {
        rig.host_game.send_readiness(GameReadiness::InPlay);
        thread::sleep(Duration::from_millis(20));
    }
    rig.guest.join(&ticket).expect("join");
    assert!(wait_until(
        || rig.guest.link_status().partner_readiness() == GameReadiness::InPlay
    ));
}

#[test]
fn a_garbage_ticket_is_refused() {
    let rig = rig();
    assert!(rig.guest.join("not a ticket").is_err());
}

pub fn drain_until(core: &LauncherCore, wanted: impl Fn(&LauncherEvent) -> bool) -> bool {
    wait_until(|| core.drain_events().iter().any(&wanted))
}

#[test]
fn a_settings_change_in_the_launcher_reaches_the_game() {
    let mut rig = rig();
    rig.host.set_player_settings(CHANGED_SETTINGS);
    assert!(wait_until(|| rig.host_game.saw(
        |m| matches!(m, Message::Settings(s) if *s == CHANGED_SETTINGS)
    )));
}

#[test]
fn a_settings_change_in_the_game_reaches_the_launcher() {
    let mut rig = rig();
    assert!(wait_until(|| {
        rig.host_game
            .send_message(&Message::Settings(CHANGED_SETTINGS));
        rig.host.drain_events().iter().any(|event| {
            matches!(
                event,
                LauncherEvent::GameChangedSettings { settings } if *settings == CHANGED_SETTINGS
            )
        })
    }));
}

#[test]
fn a_save_in_the_game_becomes_an_event_and_never_reaches_the_partner() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| rig
        .guest_game
        .saw(|m| matches!(m, Message::Welcome(_)))));
    let after_the_save = Message::EnemyGone(EnemyGone {
        phase: 3,
        id: EnemyId {
            group: 41,
            ordinal: 2,
        },
        reason: EnemyGoneReason::Removed,
    });
    let mut saved = false;
    assert!(wait_until(|| {
        rig.host_game
            .send_message(&Message::GameSaved { succeeded: true });
        rig.host_game.send_message(&after_the_save);
        saved |= rig
            .host
            .drain_events()
            .iter()
            .any(|event| matches!(event, LauncherEvent::GameSaved { succeeded: true }));
        saved && rig.guest_game.saw(|m| *m == after_the_save)
    }));
    assert!(
        !rig.guest_game
            .saw(|m| matches!(m, Message::GameSaved { .. }))
    );
}

#[test]
fn the_host_changing_its_options_in_game_updates_the_launcher_and_the_guest() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| rig
        .host_game
        .saw(|m| matches!(m, Message::Welcome(_)))));
    let options = SessionOptions {
        show_names: false,
        show_party_frame: false,
        show_distance: true,
        show_ping: true,
        ..SessionOptions::default()
    };
    rig.host_game
        .send_message(&Message::SessionOptions(options));
    assert!(drain_until(&rig.host, |event| matches!(
        event,
        LauncherEvent::SessionOptionsChanged { options: o } if *o == options
    )));
    assert!(wait_until(|| rig.guest_game.saw(
        |m| matches!(m, Message::SessionOptions(o) if *o == options)
    )));
    assert!(drain_until(&rig.guest, |event| matches!(
        event,
        LauncherEvent::SessionOptionsChanged { options: o } if *o == options
    )));
}

#[test]
fn the_guest_hears_unchanged_host_options_only_once() {
    let mut rig = rig();
    connect(&mut rig);
    let options = SessionOptions {
        show_names: false,
        ..SessionOptions::default()
    };
    rig.host_game
        .send_message(&Message::SessionOptions(options));
    thread::sleep(Duration::from_millis(1800));
    let announcements = rig
        .guest
        .drain_events()
        .iter()
        .filter(|event| matches!(event, LauncherEvent::SessionOptionsChanged { .. }))
        .count();
    assert_eq!(announcements, 1);
}

#[test]
fn a_guest_game_cannot_change_the_host_only_options() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| rig
        .guest_game
        .saw(|m| matches!(m, Message::Welcome(_)))));
    assert!(rig.guest.link_status().guest);
    rig.guest_game
        .send_message(&Message::SessionOptions(SessionOptions {
            show_names: false,
            show_party_frame: false,
            show_distance: true,
            show_ping: true,
            ..SessionOptions::default()
        }));
    thread::sleep(Duration::from_millis(300));
    assert!(!rig.guest.drain_events().iter().any(|event| matches!(
        event,
        LauncherEvent::SessionOptionsChanged { options } if !options.show_names
    )));
}

#[test]
fn a_guest_game_that_closes_tells_the_hosts_game_at_once() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| {
        rig.guest_game.send_readiness(GameReadiness::InPlay);
        rig.host_game
            .saw(|m| *m == Message::Readiness(GameReadiness::InPlay))
    }));
    let Rig {
        mut host_game,
        guest_game,
        ..
    } = rig;
    drop(guest_game);
    assert!(wait_until(
        || host_game.saw(|m| *m == Message::Readiness(GameReadiness::NotLinked))
    ));
}

#[test]
fn a_launcher_that_shuts_down_says_goodbye_to_the_other_players_game() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| rig
        .host_game
        .saw(|m| matches!(m, Message::Welcome(_)))));
    rig.guest.shutdown();
    assert!(wait_until(|| rig
        .host_game
        .saw(|m| *m == Message::MemberLeft { slot: 1 })));
}

#[test]
fn a_host_launcher_that_shuts_down_says_goodbye_to_the_guests_game() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| rig
        .guest_game
        .saw(|m| matches!(m, Message::Welcome(_)))));
    rig.host.shutdown();
    assert!(wait_until(|| rig.guest_game.saw(|m| *m == Message::Bye)));
}

#[test]
fn the_guests_launcher_gets_the_lobby_start_and_its_game_gets_nothing() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| rig
        .guest_game
        .saw(|m| matches!(m, Message::Welcome(_)))));
    assert!(wait_until(|| matches!(
        rig.host.status().state,
        NetState::Connected { .. }
    )));
    rig.host.start_lobby(LOBBY_MODE_NEW_GAME);
    assert!(drain_until(&rig.guest, |event| matches!(
        event,
        LauncherEvent::LobbyStartReceived { mode } if *mode == LOBBY_MODE_NEW_GAME
    )));
    thread::sleep(Duration::from_millis(200));
    assert!(!rig.guest_game.saw(|m| matches!(m, Message::LobbyStart(_))));
    assert!(!rig.host_game.saw(|m| matches!(m, Message::LobbyStart(_))));
}

#[test]
fn a_partners_lobby_ready_reaches_the_launcher_and_not_the_game() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| matches!(
        rig.host.status().state,
        NetState::Connected { .. }
    )));
    rig.guest.send_lobby_ready(GUEST_READY_NEW_GAME);
    assert!(drain_until(&rig.host, |event| matches!(
        event,
        LauncherEvent::MemberChanged { change: MemberChange::Lobby(ready), .. } if *ready == GUEST_READY_NEW_GAME
    )));
    assert_eq!(partner_lobby(&rig.host), Some(GUEST_READY_NEW_GAME));
    thread::sleep(Duration::from_millis(200));
    assert!(!rig.host_game.saw(|m| matches!(m, Message::LobbyReady(_))));
}

#[test]
fn a_lobby_ready_set_before_the_partner_joins_is_sent_when_it_arrives() {
    let rig = rig();
    let ticket = rig.host.host().expect("ticket");
    rig.host.send_lobby_ready(GUEST_READY_NEW_GAME);
    rig.guest.join(&ticket).expect("join");
    assert!(wait_until(
        || partner_lobby(&rig.guest) == Some(GUEST_READY_NEW_GAME)
    ));
}

#[test]
fn a_guests_pick_reaches_the_hosts_launcher_and_not_its_game() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| matches!(
        rig.host.status().state,
        NetState::Connected { .. }
    )));
    rig.guest.send_lobby_body(PartnerBody::PrologueCharacter);
    assert!(drain_until(&rig.host, |event| matches!(
        event,
        LauncherEvent::MemberChanged { change: MemberChange::Body(body), .. } if *body == PartnerBody::PrologueCharacter
    )));
    assert_eq!(
        partner_body(&rig.host),
        Some(PartnerBody::PrologueCharacter)
    );
    thread::sleep(Duration::from_millis(200));
    assert!(!rig.host_game.saw(|m| matches!(m, Message::LobbyBody(_))));
}

#[test]
fn a_pick_set_before_joining_is_sent_when_the_guest_arrives() {
    let rig = rig();
    let ticket = rig.host.host().expect("ticket");
    rig.guest.send_lobby_body(PartnerBody::PrologueCharacter);
    rig.guest.join(&ticket).expect("join");
    assert!(wait_until(
        || partner_body(&rig.host) == Some(PartnerBody::PrologueCharacter)
    ));
}

#[test]
fn a_lobby_of_two_has_a_partner() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| rig.host.link_status().partner().is_some()
        && rig.guest.link_status().partner().is_some()));
    let host_view = rig.host.link_status();
    let partner = host_view.partner().expect("partner");
    assert_eq!(partner.name, "Guesty");
    assert_eq!(partner.slot, 1);
    assert_eq!(host_view.members.len(), 1);
    let guest_view = rig.guest.link_status();
    assert_eq!(
        guest_view.partner().map(|partner| partner.name.as_str()),
        Some("Hosty")
    );
    assert!(drain_until(&rig.host, |event| matches!(
        event,
        LauncherEvent::MemberJoined { slot: 1, name } if name == "Guesty"
    )));
}

#[test]
fn a_lobby_of_three_lists_three_members() {
    let mut members = Members::default();
    members.admit(0, "Hosty".to_owned(), PeerId([1; 32]));
    members.admit(1, "Guesty".to_owned(), PeerId([2; 32]));
    members.admit(2, "Third".to_owned(), PeerId([3; 32]));
    let mut status = LinkStatus::default();
    members.write_status(&mut status);
    let slots: Vec<u8> = status.members.iter().map(|row| row.slot).collect();
    assert_eq!(slots, vec![0, 1, 2]);
    assert!(status.partner().is_none());
    assert_eq!(status.partner_readiness(), GameReadiness::NotLinked);
    for row in &mut status.members {
        if row.slot == 2 {
            row.apply(MemberChange::Lobby(GUEST_READY_NEW_GAME));
            row.apply(MemberChange::Readiness(GameReadiness::InPlay));
        }
    }
    members.write_status(&mut status);
    let third = status.member(2).expect("third");
    assert_eq!(third.name, "Third");
    assert_eq!(third.lobby, Some(GUEST_READY_NEW_GAME));
    assert_eq!(third.readiness, GameReadiness::InPlay);
    assert_eq!(status.member(1).expect("second").lobby, None);
    members.leave(1);
    members.write_status(&mut status);
    assert_eq!(status.members.len(), 2);
    members.leave(0);
    members.write_status(&mut status);
    assert_eq!(status.partner().map(|partner| partner.slot), Some(2));
    assert_eq!(status.partner_readiness(), GameReadiness::InPlay);
    let left = LauncherEvent::MemberLeft {
        slot: 2,
        detail: String::new(),
    };
    assert!(matches!(left, LauncherEvent::MemberLeft { slot: 2, .. }));
}

#[test]
fn the_hosts_rules_reach_every_launcher() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| rig.guest.link_status().partner().is_some()));
    assert!(wait_until(|| rig
        .guest_game
        .saw(|m| matches!(m, Message::Welcome(_)))));
    let rules = SessionRules {
        mode: STORY_MODE + 1,
        max_players: 2,
    };
    rig.host.set_rules(rules);
    assert!(wait_until(|| rig.guest.link_status().rules == rules));
    assert_eq!(rig.host.link_status().rules, rules);
    assert!(wait_until(|| rig
        .guest_game
        .saw(|m| matches!(m, Message::Rules(seen) if *seen == rules))));
}

#[test]
fn a_guest_cannot_start_the_lobby() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| matches!(
        rig.guest.status().state,
        NetState::Connected { .. }
    )));
    rig.guest.start_lobby(LOBBY_MODE_NEW_GAME);
    thread::sleep(Duration::from_millis(300));
    assert!(
        !rig.host
            .drain_events()
            .iter()
            .any(|event| matches!(event, LauncherEvent::LobbyStartReceived { .. }))
    );
}

#[test]
fn the_launcher_asking_its_game_to_quit_reaches_only_that_game() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| {
        rig.host_game.pump();
        rig.host.link_status().game_linked
    }));
    rig.host.ask_game_to_quit();
    assert!(wait_until(|| rig
        .host_game
        .saw(|m| *m == Message::QuitGame)));
    thread::sleep(Duration::from_millis(200));
    assert!(!rig.guest_game.saw(|m| *m == Message::QuitGame));
}

#[test]
fn a_partners_frames_reach_the_game_with_the_partners_slot() {
    let mut rig = rig();
    connect(&mut rig);
    let mut tick = 0;
    assert!(wait_until(|| {
        tick += 1;
        rig.host_game.send_state(&state_at(tick, 7.0));
        rig.guest_game
            .sender_of(|m| matches!(m, Message::State(s) if s.position[0] == 7.0))
            == Some(0)
    }));
    assert!(wait_until(|| {
        tick += 1;
        rig.guest_game.send_state(&state_at(tick, 9.0));
        rig.host_game
            .sender_of(|m| matches!(m, Message::State(s) if s.position[0] == 9.0))
            == Some(1)
    }));
}

#[test]
fn the_launchers_own_frames_to_the_game_carry_the_players_own_slot() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| rig
        .guest_game
        .sender_of(|m| matches!(m, Message::Welcome(_)))
        == Some(1)));
    assert!(wait_until(|| rig
        .guest_game
        .sender_of(|m| matches!(m, Message::PathInfo(_)))
        == Some(1)));
}

fn member_ping(core: &LauncherCore, slot: u8) -> Option<u32> {
    core.link_status().member(slot)?.rtt_ms
}

#[test]
fn a_member_has_a_ping_once_the_host_has_measured_it() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| rig.host.link_status().partner().is_some()));
    assert_eq!(member_ping(&rig.host, 1), None);
    assert!(wait_until(|| member_ping(&rig.host, 1).is_some()));
    assert!(wait_until(|| member_ping(&rig.guest, 0) == Some(0)));
}

#[test]
fn the_removed_launcher_says_the_host_removed_it() {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| rig.host.link_status().partner().is_some()
        && rig.guest.link_status().partner().is_some()));
    rig.guest.remove_member(0);
    rig.host.remove_member(1);
    assert!(wait_until(|| rig.host.link_status().members.is_empty()));
    assert!(wait_until(|| matches!(
        rig.guest.status().state,
        NetState::Failed(ref text) if text == "The host removed you from the lobby."
    )));
    assert!(drain_until(&rig.guest, |event| matches!(
        event,
        LauncherEvent::MemberLeft { slot: 0, detail } if detail.starts_with("removed by the host")
    )));
}

fn campaign_card() -> CampaignCard {
    CampaignCard {
        campaign_id: "0123456789abcdef0123456789abcdef".to_owned(),
        host: SaveSummary {
            name: "Nier".to_owned(),
            level: 21,
            play_seconds: 7384.5,
        },
        second_seat: Some(SecondSeatCard {
            summary: SaveSummary {
                name: "Kaine".to_owned(),
                level: 19,
                play_seconds: 6000.0,
            },
            body: Some(PartnerBody::PrologueCharacter),
        }),
    }
}

fn connected_rig() -> Rig {
    let mut rig = rig();
    connect(&mut rig);
    assert!(wait_until(|| matches!(
        rig.host.status().state,
        NetState::Connected { .. }
    ) && rig
        .guest
        .link_status()
        .partner()
        .is_some()));
    rig
}

fn received_seat(events: &[LauncherEvent]) -> Option<(u8, u32, Vec<u8>)> {
    events.iter().find_map(|event| match event {
        LauncherEvent::SeatReceived {
            slot,
            transfer,
            seat,
        } => Some((*slot, *transfer, seat.clone())),
        _ => None,
    })
}

#[test]
fn a_seat_reaches_the_guest_whole() {
    let rig = connected_rig();
    let seat = generated_seat(3);
    rig.host.send_seat(1, seat.clone());
    let mut guest_events = Vec::new();
    assert!(wait_until(|| {
        guest_events.extend(rig.guest.drain_events());
        received_seat(&guest_events).is_some()
    }));
    let (from, _, arrived) = received_seat(&guest_events).expect("arrived");
    assert_eq!((from, arrived), (0, seat));
    assert!(guest_events.iter().any(|event| matches!(
        event,
        LauncherEvent::SeatProgress {
            incoming: true,
            parts: 79,
            ..
        }
    )));
    assert!(drain_until(&rig.host, |event| matches!(
        event,
        LauncherEvent::SeatDelivered { slot: 1, .. }
    )));
}

#[test]
fn a_seat_comes_back_to_the_host_after_a_save() {
    let rig = connected_rig();
    let first = generated_seat(3);
    rig.host.send_seat(1, first);
    assert!(drain_until(&rig.guest, |event| matches!(
        event,
        LauncherEvent::SeatReceived { .. }
    )));
    let saved = generated_seat(5);
    rig.guest.send_seat(0, saved.clone());
    let mut host_events = Vec::new();
    assert!(wait_until(|| {
        host_events.extend(rig.host.drain_events());
        received_seat(&host_events).is_some()
    }));
    assert_eq!(
        received_seat(&host_events).map(|(slot, _, seat)| (slot, seat)),
        Some((1, saved))
    );
    assert!(drain_until(&rig.guest, |event| matches!(
        event,
        LauncherEvent::SeatDelivered { slot: 0, .. }
    )));
}

#[test]
fn the_games_never_hear_of_a_seat_or_a_card() {
    let mut rig = connected_rig();
    rig.host.send_campaign_card(campaign_card());
    rig.host.send_seat(1, generated_seat(3));
    assert!(drain_until(&rig.guest, |event| matches!(
        event,
        LauncherEvent::SeatReceived { .. }
    )));
    thread::sleep(Duration::from_millis(200));
    for game in [&mut rig.host_game, &mut rig.guest_game] {
        game.pump();
        assert!(!game.saw(|m| matches!(
            m,
            Message::CampaignCard(_) | Message::SeatPart(_) | Message::SeatReceipt(_)
        )));
    }
}

#[test]
fn the_guest_is_shown_the_campaign_before_the_start() {
    let rig = connected_rig();
    rig.host.send_campaign_card(campaign_card());
    let mut guest_events = Vec::new();
    assert!(wait_until(|| {
        guest_events.extend(rig.guest.drain_events());
        guest_events
            .iter()
            .any(|event| matches!(event, LauncherEvent::CampaignCardReceived { .. }))
    }));
    rig.host.start_lobby(LOBBY_MODE_NEW_GAME);
    assert!(wait_until(|| {
        guest_events.extend(rig.guest.drain_events());
        guest_events
            .iter()
            .any(|event| matches!(event, LauncherEvent::LobbyStartReceived { .. }))
    }));
    let card_at = guest_events
        .iter()
        .position(|event| matches!(event, LauncherEvent::CampaignCardReceived { .. }));
    let start_at = guest_events
        .iter()
        .position(|event| matches!(event, LauncherEvent::LobbyStartReceived { .. }));
    assert!(card_at < start_at);
    assert!(guest_events.contains(&LauncherEvent::CampaignCardReceived {
        card: campaign_card()
    }));
}

#[test]
fn a_card_set_before_the_guest_joins_is_sent_when_it_arrives() {
    let rig = rig();
    let ticket = rig.host.host().expect("ticket");
    rig.host.send_campaign_card(campaign_card());
    rig.guest.join(&ticket).expect("join");
    assert!(drain_until(&rig.guest, |event| matches!(
        event,
        LauncherEvent::CampaignCardReceived { .. }
    )));
}

#[test]
fn a_seat_that_is_not_a_save_is_not_sent() {
    let rig = connected_rig();
    rig.host.send_seat(1, vec![0; 100]);
    assert!(drain_until(&rig.host, |event| matches!(
        event,
        LauncherEvent::SeatTransferRefused {
            reason: SeatRefusal::WrongSize,
            by_peer: false,
            ..
        }
    )));
}

type RawSession = Session<LoopbackTransport, Message>;

fn raw_guest(network: &LoopbackNetwork, ticket: &str) -> RawSession {
    let mut guest: RawSession = Session::new(
        network.endpoint(),
        SessionConfig {
            role: Role::Guest,
            name: "Raw".to_owned(),
            build: LauncherConfig::new("x").build,
            rules: SessionRules {
                mode: STORY_MODE,
                max_players: 2,
            },
        },
    );
    let invite = Invite::new(ticket.to_owned()).expect("invite");
    guest.join(&invite).expect("join");
    let mut events = Vec::new();
    assert!(wait_until(|| {
        guest.poll(Instant::now(), &mut events);
        events
            .iter()
            .any(|event| matches!(event, SessionEvent::PlayerJoined { .. }))
    }));
    guest
}

fn next_receipt(guest: &mut RawSession) -> Option<SeatReceipt> {
    let mut found = None;
    let mut events = Vec::new();
    wait_until(|| {
        guest.poll(Instant::now(), &mut events);
        found = events.iter().find_map(|event| match event {
            SessionEvent::Message {
                message: Message::SeatReceipt(receipt),
                ..
            } => Some(*receipt),
            _ => None,
        });
        found.is_some()
    });
    found
}

#[test]
fn a_part_out_of_place_is_refused_and_nothing_is_written() {
    let network = LoopbackNetwork::new();
    let host =
        LauncherCore::new(LauncherConfig::new("Hosty"), loopback_factory(&network)).expect("core");
    let ticket = host.host().expect("ticket");
    let mut guest = raw_guest(&network, &ticket);
    let parts = all_parts(1, generated_seat(3));
    for part in [parts[0].clone(), parts[2].clone()] {
        guest
            .send_relayed_to(0, &Message::SeatPart(part))
            .expect("valid part");
    }
    assert_eq!(
        next_receipt(&mut guest),
        Some(SeatReceipt {
            transfer: 1,
            outcome: SeatOutcome::Refused(SeatRefusal::OutOfOrder)
        })
    );
    guest
        .send_relayed_to(0, &Message::SeatPart(parts[1].clone()))
        .expect("valid part");
    thread::sleep(Duration::from_millis(200));
    let events = host.drain_events();
    assert!(received_seat(&events).is_none());
    assert!(events.iter().any(|event| matches!(
        event,
        LauncherEvent::SeatTransferRefused {
            slot: 1,
            reason: SeatRefusal::OutOfOrder,
            by_peer: false,
            ..
        }
    )));
}

#[test]
fn a_damaged_seat_is_refused_whole_and_nothing_is_written() {
    let network = LoopbackNetwork::new();
    let host =
        LauncherCore::new(LauncherConfig::new("Hosty"), loopback_factory(&network)).expect("core");
    let ticket = host.host().expect("ticket");
    let mut guest = raw_guest(&network, &ticket);
    let mut parts = all_parts(2, generated_seat(3));
    parts[30].bytes[0] ^= 0xFF;
    for part in parts {
        guest
            .send_relayed_to(0, &Message::SeatPart(part))
            .expect("valid part");
    }
    assert_eq!(
        next_receipt(&mut guest),
        Some(SeatReceipt {
            transfer: 2,
            outcome: SeatOutcome::Refused(SeatRefusal::BadChecksum)
        })
    );
    assert!(received_seat(&host.drain_events()).is_none());
}
