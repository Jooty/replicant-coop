use std::thread;
use std::time::{Duration, Instant};

use nc_net::{Channel, LINK_GAME_PEER, LocalLinkTransport, Transport, TransportEvent};
use nc_wire::{Envelope, STORY_MODE, SessionRules, decode_enveloped, encode_enveloped};
use replicant_hook::net::NetSharedExt;
use replicant_hook::net::{LocalSnapshot, NetState, NetThread};
use replicant_proto::{
    BuildGate, CombatEvent, EnemyGone, EnemyGoneReason, EnemyId, FollowPhase, GameReadiness,
    Member, Message, PathInfo, PlayerSettings, PlayerState, Reject, RejectReason, SessionOptions,
    Welcome,
};

const BUILD: BuildGate = BuildGate {
    time_date_stamp: 0x60B7_3C00,
    size_of_image: 0x0533_9000,
};
const GAME_SLOT: u8 = 1;
const PARTNER_SLOT: u8 = 0;
const WAIT_LIMIT: Duration = Duration::from_secs(20);
const LAUNCHER_SETTINGS: PlayerSettings = PlayerSettings {
    keep_running: false,
    event_log: false,
};

struct FakeLauncher {
    link: LocalLinkTransport,
    received: Vec<Message>,
}

impl FakeLauncher {
    fn start() -> (Self, u16) {
        let mut link = LocalLinkTransport::launcher(Some(BUILD));
        link.host().expect("listens");
        let port = link.local_port().expect("bound");
        (
            Self {
                link,
                received: Vec::new(),
            },
            port,
        )
    }

    fn pump(&mut self) {
        let mut events = Vec::new();
        self.link.poll(&mut events);
        for event in events {
            if let TransportEvent::Message { payload, .. } = event
                && let Ok((_, message)) = decode_enveloped::<Message>(&payload)
            {
                self.received.push(message);
            }
        }
    }

    fn say(&mut self, message: &Message) {
        self.say_from(GAME_SLOT, message);
    }

    fn say_relayed(&mut self, message: &Message) {
        self.say_from(PARTNER_SLOT, message);
    }

    fn say_from(&mut self, from: u8, message: &Message) {
        let envelope = Envelope { from, to: None };
        let bytes = encode_enveloped(&envelope, message).expect("encodes");
        let _ = self
            .link
            .send(LINK_GAME_PEER, Channel::ReliableOrdered, &bytes);
    }

    fn wait_until(&mut self, mut condition: impl FnMut(&mut Self) -> bool) -> bool {
        let deadline = Instant::now() + WAIT_LIMIT;
        while Instant::now() < deadline {
            self.pump();
            if condition(self) {
                return true;
            }
            thread::sleep(Duration::from_millis(2));
        }
        false
    }
}

fn start_game() -> (FakeLauncher, NetThread) {
    let (launcher, port) = FakeLauncher::start();
    let game = NetThread::spawn(LocalLinkTransport::game(BUILD), port).expect("net thread starts");
    (launcher, game)
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

fn snapshot_at(x: f32) -> LocalSnapshot {
    LocalSnapshot {
        phase: 3,
        position: [x, 0.0, 0.0],
        yaw_radians: 0.0,
        hp: 10,
        max_hp: 10,
        body_motion: None,
        body_motion_hash: 0,
        weapon: None,
        activity: Default::default(),
        mp: 0.0,
        max_mp: 0.0,
        level: 0,
        ridden_boar: None,
    }
}

fn remote_state(x: f32) -> PlayerState {
    snapshot_at(x).to_player_state(1)
}

fn member(slot: u8, name: &str) -> Message {
    Message::Member(Member {
        slot,
        name: name.to_owned(),
    })
}

fn welcome(slot: u8, name: &str) -> Message {
    Message::Welcome(Welcome {
        version: replicant_proto::PROTOCOL_VERSION,
        slot,
        host_name: name.to_owned(),
        rules: SessionRules {
            mode: STORY_MODE,
            max_players: 2,
        },
    })
}

fn link_up_partner_loading(launcher: &mut FakeLauncher, game: &NetThread) {
    assert!(wait_until(|| game.view().state == NetState::LauncherLinked));
    launcher.say(&welcome(GAME_SLOT, "Hosty"));
    launcher.say(&member(PARTNER_SLOT, "Hosty"));
    assert!(wait_until(|| game.view().state == NetState::Connected));
}

fn link_up(launcher: &mut FakeLauncher, game: &NetThread) {
    link_up_partner_loading(launcher, game);
    launcher.say_relayed(&Message::Readiness(GameReadiness::InPlay));
    assert!(wait_until(|| game.view().partner_in_play()));
}

#[test]
fn the_game_waits_for_the_launcher_then_links() {
    let (mut launcher, port) = FakeLauncher::start();
    let game = NetThread::spawn(LocalLinkTransport::game(BUILD), port).expect("starts");
    assert!(wait_until(|| game.view().state == NetState::LauncherLinked));
    assert_eq!(game.view().link_port, Some(port));
    assert!(!game.wants_snapshots());
    launcher.pump();
}

#[test]
fn without_a_listening_launcher_the_state_stays_waiting() {
    let (launcher, port) = FakeLauncher::start();
    drop(launcher);
    let game = NetThread::spawn(LocalLinkTransport::game(BUILD), port).expect("starts");
    thread::sleep(Duration::from_millis(300));
    assert_eq!(game.view().state, NetState::WaitingForLauncher);
}

#[test]
fn a_welcome_from_the_launcher_names_the_peer_and_starts_snapshots() {
    let (mut launcher, game) = start_game();
    link_up(&mut launcher, &game);
    assert_eq!(game.view().partner_name(), Some("Hosty"));
    assert!(game.wants_snapshots());
}

#[test]
fn relayed_player_state_reaches_the_store() {
    let (mut launcher, game) = start_game();
    link_up(&mut launcher, &game);
    launcher.say_relayed(&Message::State(remote_state(42.0)));
    assert!(wait_until(|| {
        game.view()
            .partner_state()
            .is_some_and(|remote| remote.state.position[0] == 42.0 && remote.state.phase == 3)
    }));
    let remote = game
        .view()
        .partner_state()
        .cloned()
        .expect("remote was just seen");
    assert_eq!(remote.name, "Hosty");
    assert_eq!(remote.slot, 0);
}

#[test]
fn published_snapshots_reach_the_launcher_as_player_state() {
    let (mut launcher, game) = start_game();
    link_up(&mut launcher, &game);
    let seen = launcher.wait_until(|launcher| {
        game.publish(snapshot_at(7.0));
        launcher.received.iter().any(|message| {
            matches!(message, Message::State(state) if state.position[0] == 7.0 && state.phase == 3)
        })
    });
    assert!(seen, "the launcher never saw the game's position");
}

fn enemy_gone(ordinal: u16) -> Message {
    Message::EnemyGone(EnemyGone {
        phase: 3,
        id: EnemyId { group: 9, ordinal },
        reason: EnemyGoneReason::Killed,
    })
}

#[test]
fn relayed_game_messages_from_the_launcher_wait_in_the_inbox() {
    let (mut launcher, game) = start_game();
    link_up(&mut launcher, &game);
    launcher.say_relayed(&enemy_gone(1));
    launcher.say_relayed(&enemy_gone(2));
    let mut taken = Vec::new();
    assert!(wait_until(|| {
        taken.extend(game.take_relayed(|message| matches!(message, Message::EnemyGone(_))));
        taken.len() >= 2
    }));
    assert_eq!(taken, vec![enemy_gone(1), enemy_gone(2)]);
    assert!(game.take_relayed(|_| true).is_empty());
}

#[test]
fn relayed_game_messages_from_the_game_reach_the_launcher() {
    let (mut launcher, game) = start_game();
    link_up(&mut launcher, &game);
    game.send_relayed(enemy_gone(4));
    assert!(launcher.wait_until(|launcher| launcher.received.contains(&enemy_gone(4))));
}

#[test]
fn path_info_fills_rtt_and_path() {
    let (mut launcher, game) = start_game();
    link_up(&mut launcher, &game);
    launcher.say(&Message::PathInfo(PathInfo {
        rtt_micros: 12_500,
        direct: false,
    }));
    assert!(wait_until(|| game.view().direct_path == Some(false)));
    assert_eq!(game.view().rtt, Some(Duration::from_micros(12_500)));
}

#[test]
fn a_bye_from_the_launcher_clears_the_peer() {
    let (mut launcher, game) = start_game();
    link_up(&mut launcher, &game);
    launcher.say_relayed(&Message::State(remote_state(1.0)));
    assert!(wait_until(|| game.view().partner_state().is_some()));
    launcher.say(&Message::Bye);
    assert!(wait_until(|| game.view().state == NetState::LauncherLinked));
    assert!(game.view().partner_state().is_none());
    assert!(!game.wants_snapshots());
}

#[test]
fn a_reject_is_shown() {
    let (mut launcher, game) = start_game();
    assert!(wait_until(|| game.view().state == NetState::LauncherLinked));
    launcher.say(&Message::Reject(Reject {
        reason: RejectReason::SessionFull,
    }));
    assert!(wait_until(|| {
        game.view().state == NetState::Rejected(RejectReason::SessionFull)
    }));
}

#[test]
fn losing_the_launcher_returns_to_waiting_and_relinks() {
    let (mut launcher, game) = start_game();
    link_up(&mut launcher, &game);
    launcher.link.disconnect(LINK_GAME_PEER);
    assert!(wait_until(|| {
        game.view().state == NetState::WaitingForLauncher
            || game.view().state == NetState::LauncherLinked
    }));
    assert!(wait_until(|| game.view().state == NetState::LauncherLinked));
    assert!(game.view().partner_state().is_none());
}

#[test]
fn combat_events_go_out_to_the_launcher_and_come_in_from_it() {
    let (mut launcher, game) = start_game();
    link_up(&mut launcher, &game);
    game.send_relayed(Message::Combat(CombatEvent::Downed));
    assert!(launcher.wait_until(|launcher| {
        launcher
            .received
            .contains(&Message::Combat(CombatEvent::Downed))
    }));
    launcher.say_relayed(&Message::Combat(CombatEvent::Revived));
    assert!(wait_until(|| {
        game.take_relayed(|message| matches!(message, Message::Combat(_)))
            .contains(&Message::Combat(CombatEvent::Revived))
    }));
}

fn follow_order() -> Message {
    Message::FollowPhase(FollowPhase::at(2, 1))
}

#[test]
fn a_partner_whose_game_is_not_in_play_is_joining_and_its_messages_are_dropped() {
    let (mut launcher, game) = start_game();
    link_up_partner_loading(&mut launcher, &game);
    launcher.say_relayed(&Message::Readiness(GameReadiness::Loading));
    launcher.say_relayed(&follow_order());
    launcher.say_relayed(&enemy_gone(1));
    launcher.say_relayed(&Message::Readiness(GameReadiness::InPlay));
    assert!(wait_until(|| game.view().partner_in_play()));
    assert!(game.view().partner_state().is_none());
    assert!(game.take_relayed(|_| true).is_empty());
}

#[test]
fn a_partner_who_is_joining_is_shown_as_joining() {
    let (mut launcher, game) = start_game();
    link_up_partner_loading(&mut launcher, &game);
    assert!(game.view().partner_joining());
    assert!(!game.view().partner_in_play());
}

#[test]
fn messages_flow_once_the_partner_reports_in_play() {
    let (mut launcher, game) = start_game();
    link_up(&mut launcher, &game);
    launcher.say_relayed(&follow_order());
    assert!(wait_until(|| {
        !game
            .take_relayed(|message| *message == follow_order())
            .is_empty()
    }));
}

#[test]
fn a_partner_game_that_leaves_play_loses_its_state_and_stale_orders() {
    let (mut launcher, game) = start_game();
    link_up(&mut launcher, &game);
    launcher.say_relayed(&Message::State(remote_state(2.0)));
    launcher.say_relayed(&follow_order());
    assert!(wait_until(|| game.view().partner_state().is_some()));
    launcher.say_relayed(&Message::Readiness(GameReadiness::NotLinked));
    assert!(wait_until(|| !game.view().partner_in_play()));
    assert!(game.view().partner_state().is_none());
    assert!(game.take_relayed(|_| true).is_empty());
    assert!(game.view().partner_joining());
}

#[test]
fn game_events_wait_while_the_partner_is_not_in_play() {
    let (mut launcher, game) = start_game();
    link_up_partner_loading(&mut launcher, &game);
    game.send_relayed(enemy_gone(8));
    thread::sleep(Duration::from_millis(300));
    launcher.pump();
    assert!(!launcher.received.contains(&enemy_gone(8)));
}

#[test]
fn the_games_readiness_report_reaches_the_launcher_and_goes_stale() {
    let (mut launcher, game) = start_game();
    link_up(&mut launcher, &game);
    game.report_readiness(GameReadiness::InPlay);
    assert!(launcher.wait_until(|launcher| {
        launcher
            .received
            .contains(&Message::Readiness(GameReadiness::InPlay))
    }));
    launcher.received.clear();
    assert!(launcher.wait_until(|launcher| {
        launcher
            .received
            .contains(&Message::Readiness(GameReadiness::Loading))
    }));
}

#[test]
fn a_game_that_never_reports_is_loading_to_the_launcher() {
    let (mut launcher, game) = start_game();
    assert!(wait_until(|| game.view().state == NetState::LauncherLinked));
    assert!(launcher.wait_until(|launcher| {
        launcher
            .received
            .contains(&Message::Readiness(GameReadiness::Loading))
    }));
}

#[test]
fn states_still_flow_when_readiness_never_arrives() {
    let (mut launcher, game) = start_game();
    link_up_partner_loading(&mut launcher, &game);
    launcher.say_relayed(&Message::State(remote_state(9.0)));
    assert!(wait_until(|| game.view().partner_state().is_some()));
    assert!(game.view().partner_in_play());
    launcher.say_relayed(&follow_order());
    assert!(wait_until(|| {
        !game
            .take_relayed(|message| *message == follow_order())
            .is_empty()
    }));
}

#[test]
fn a_partner_that_never_reports_is_assumed_in_play_after_a_few_seconds() {
    let (mut launcher, game) = start_game();
    link_up_partner_loading(&mut launcher, &game);
    assert!(!game.view().partner_in_play());
    assert!(wait_until(|| game.view().partner_in_play()));
}

#[test]
fn settings_from_the_launcher_wait_until_the_game_takes_them_once() {
    let (mut launcher, game) = start_game();
    link_up(&mut launcher, &game);
    launcher.say(&Message::Settings(LAUNCHER_SETTINGS));
    let mut taken = None;
    assert!(wait_until(|| {
        taken = game.take_launcher_settings();
        taken.is_some()
    }));
    assert_eq!(taken, Some(LAUNCHER_SETTINGS));
    assert_eq!(game.take_launcher_settings(), None);
}

#[test]
fn a_settings_change_made_in_game_reaches_the_launcher() {
    let (mut launcher, game) = start_game();
    link_up(&mut launcher, &game);
    let seen = launcher.wait_until(|launcher| {
        game.tell_launcher_settings(LAUNCHER_SETTINGS);
        launcher
            .received
            .contains(&Message::Settings(LAUNCHER_SETTINGS))
    });
    assert!(seen, "the launcher never saw the settings");
}

#[test]
fn host_options_changed_in_game_update_the_view_and_reach_the_launcher() {
    let (mut launcher, game) = start_game();
    link_up(&mut launcher, &game);
    let options = SessionOptions {
        show_names: false,
        show_party_frame: true,
        show_distance: true,
        show_ping: true,
        ..SessionOptions::default()
    };
    game.tell_launcher_host_options(options);
    assert_eq!(game.view().options, options);
    let seen = launcher.wait_until(|launcher| {
        launcher
            .received
            .contains(&Message::SessionOptions(options))
    });
    assert!(seen, "the launcher never saw the host options");
}

#[test]
fn a_finished_save_reaches_the_launcher_with_its_result() {
    let (mut launcher, game) = start_game();
    link_up(&mut launcher, &game);
    game.tell_launcher_saved(true);
    game.tell_launcher_saved(false);
    let seen = launcher.wait_until(|launcher| {
        launcher
            .received
            .contains(&Message::GameSaved { succeeded: false })
    });
    assert!(seen, "the launcher never saw the failed save");
    assert!(
        launcher
            .received
            .contains(&Message::GameSaved { succeeded: true })
    );
}
