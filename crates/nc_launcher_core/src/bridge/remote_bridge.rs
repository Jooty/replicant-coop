use std::collections::BTreeMap;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use nc_net::link_traffic::{LinkTraffic, RefusalLog};
use nc_net::{
    Channel, Invite, LINK_GAME_PEER, LeaveReason, LinkProgress, LocalLinkTransport, ProgressBoard,
    Role, Session, SessionConfig, SessionEvent, Transport, TransportEvent,
};
use nc_wire::{Envelope, SessionRules, decode_enveloped, encode_enveloped};
use replicant_game::readiness::LocalGameReadiness;
use replicant_proto::{
    CampaignCard, GameReadiness, LobbyBody, LobbyReady, LobbyStart, Message, PROTOCOL_VERSION,
    PartnerBody, PathInfo, PlayerSettings, RejectReason, SeatOutcome, SeatPart, SeatReceipt,
    SessionOptions, Welcome, WireError,
};

use crate::bridge::members::Members;
use crate::event::options_differ_from_last;
use crate::seat_transfer::AssemblerStep;
use crate::{
    ConnectionState, HOST_ANSWER_TIMEOUT, HOST_KEY_NOT_FOUND_TEXT, HOST_UNREACHABLE_TEXT,
    INVALID_TICKET_TEXT, LauncherConfig, LauncherEvent, LinkStatus, MemberChange, NetError,
    PATH_INFO_INTERVAL, SEAT_PARTS_PER_PUMP, STORY_RULES, SeatAssembler, SeatSender,
    SessionEndCause, TransportFactory,
};

type RemoteSession = Session<Box<dyn Transport>, Message>;

pub(crate) struct Bridge {
    link: LocalLinkTransport,
    factory: TransportFactory,
    config: LauncherConfig,
    status: Arc<Mutex<LinkStatus>>,
    events: Sender<LauncherEvent>,
    remote: Option<RemoteSession>,
    members: Members,
    last_path_info: Option<Instant>,
    last_reject: Option<RejectReason>,
    options: SessionOptions,
    rules: SessionRules,
    last_options_announced: Option<SessionOptions>,
    settings: Option<PlayerSettings>,
    local_game: LocalGameReadiness,
    readiness_sent: Option<GameReadiness>,
    own_readiness_logged: Option<GameReadiness>,
    own_lobby: Option<LobbyReady>,
    own_body: Option<PartnerBody>,
    own_card: Option<CampaignCard>,
    incoming_seats: BTreeMap<u8, SeatAssembler>,
    outgoing_seats: BTreeMap<u8, SeatSender>,
    traffic: LinkTraffic,
    refusals: RefusalLog,
    progress: ProgressBoard,
    progress_logged: usize,
    session_asked_at: Option<Instant>,
}

impl Bridge {
    pub(crate) fn new(
        link: LocalLinkTransport,
        factory: TransportFactory,
        config: LauncherConfig,
        status: Arc<Mutex<LinkStatus>>,
        events: Sender<LauncherEvent>,
        progress: ProgressBoard,
    ) -> Self {
        Self {
            link,
            factory,
            config,
            status,
            events,
            remote: None,
            members: Members::default(),
            last_path_info: None,
            last_reject: None,
            options: SessionOptions::default(),
            rules: STORY_RULES,
            last_options_announced: None,
            settings: None,
            local_game: LocalGameReadiness::new(),
            readiness_sent: None,
            own_readiness_logged: None,
            own_lobby: None,
            own_body: None,
            own_card: None,
            incoming_seats: BTreeMap::new(),
            outgoing_seats: BTreeMap::new(),
            traffic: LinkTraffic::default(),
            refusals: RefusalLog::default(),
            progress,
            progress_logged: 0,
            session_asked_at: None,
        }
    }

    pub(crate) fn host(&mut self) -> Result<String, NetError> {
        self.end_session(SessionEndCause::Replaced);
        let mut session = self.new_session(Role::Host)?;
        let invite = session
            .host()
            .map_err(|error| NetError::Failed(error.to_string()))?;
        let ticket = invite.as_str().to_owned();
        let rules = session.rules();
        self.remote = Some(session);
        let mut status = self.status();
        status.rules = rules;
        status.guest = false;
        status.own_slot = Some(0);
        status.state = ConnectionState::Hosting;
        status.ticket = Some(ticket.clone());
        drop(status);
        self.emit(LauncherEvent::Hosting {
            ticket: ticket.clone(),
        });
        Ok(ticket)
    }

    pub(crate) fn join(&mut self, ticket: &str) -> Result<(), NetError> {
        let invite = Invite::new(ticket.trim().to_owned())
            .map_err(|_| NetError::Failed(INVALID_TICKET_TEXT.to_owned()))?;
        self.end_session(SessionEndCause::Replaced);
        let mut session = self.new_session(Role::Guest)?;
        session
            .join(&invite)
            .map_err(|_| NetError::Failed(INVALID_TICKET_TEXT.to_owned()))?;
        self.remote = Some(session);
        let mut status = self.status();
        status.guest = true;
        status.state = ConnectionState::Joining;
        drop(status);
        self.emit(LauncherEvent::Joining);
        Ok(())
    }

    pub(crate) fn end_session(&mut self, cause: SessionEndCause) {
        if self.remote.is_none() {
            return;
        }
        let last_seen = self.status().clone();
        self.stop();
        self.emit(LauncherEvent::SessionEnded { cause, last_seen });
    }

    pub(crate) fn stop(&mut self) {
        self.last_reject = None;
        self.session_asked_at = None;
        let slots: Vec<u8> = self.members.all().map(|member| member.slot).collect();
        if let Some(mut session) = self.remote.take() {
            session.close();
        }
        self.forget_peer();
        if !slots.is_empty() {
            self.send_to_game(&Message::Bye);
            let detail = format!(
                "you closed the link; {}",
                self.traffic.describe(Instant::now())
            );
            for slot in slots {
                self.emit(LauncherEvent::MemberLeft {
                    slot,
                    detail: detail.clone(),
                });
            }
        }
        let mut status = self.status();
        status.state = ConnectionState::Idle;
        status.guest = false;
        status.own_slot = None;
        status.ticket = None;
    }

    pub(crate) fn set_options(&mut self, options: SessionOptions) {
        self.options = options;
        if !self.status().guest {
            self.last_options_announced = Some(options);
        }
        self.send_options_if_hosting();
    }

    pub(crate) fn set_rules(&mut self, rules: SessionRules) {
        self.rules = rules;
        if let Some(session) = self.remote.as_mut() {
            session.set_rules(rules);
            let applied = session.rules();
            self.status().rules = applied;
        }
    }

    pub(crate) fn set_settings(&mut self, settings: PlayerSettings) {
        self.settings = Some(settings);
        self.send_to_game(&Message::Settings(settings));
    }

    pub(crate) fn set_lobby_ready(&mut self, ready: LobbyReady) {
        self.own_lobby = Some(ready);
        self.send_own_lobby();
    }

    pub(crate) fn set_lobby_body(&mut self, body: PartnerBody) {
        self.own_body = Some(body);
        self.send_own_body();
    }

    pub(crate) fn set_campaign_card(&mut self, card: CampaignCard) {
        self.own_card = Some(card);
        self.send_own_card();
    }

    pub(crate) fn send_seat(&mut self, slot: u8, seat: Vec<u8>, transfer: u32) {
        if !self.members.is_member(slot) {
            self.report_refused("SeatPart", &WireError::OutOfRange { field: "slot" });
            return;
        }
        match SeatSender::new(transfer, seat) {
            Ok(sender) => {
                self.outgoing_seats.insert(slot, sender);
            }
            Err(reason) => self.emit(LauncherEvent::SeatTransferRefused {
                slot,
                transfer,
                reason,
                by_peer: false,
            }),
        }
    }

    pub(crate) fn quit_game(&mut self) {
        self.send_to_game(&Message::QuitGame);
    }

    pub(crate) fn start_lobby(&mut self, mode: u8) {
        if self.status().guest {
            return;
        }
        if self.members.is_empty() {
            return;
        }
        let Some(session) = self.remote.as_mut() else {
            return;
        };
        let _ = session.send_relayed(&Message::LobbyStart(LobbyStart { mode }));
    }

    pub(crate) fn remove_member(&mut self, slot: u8) {
        if self.status().guest {
            return;
        }
        if let Some(session) = self.remote.as_mut() {
            session.remove(slot);
        }
    }

    pub(crate) fn set_name(&mut self, name: &str) {
        self.config = LauncherConfig::new(name);
        if let Some(session) = self.remote.as_mut() {
            session.set_name(&self.config.name);
        }
    }

    pub(crate) fn pump(&mut self, now: Instant) {
        self.pump_link();
        self.pump_remote(now);
        self.log_progress();
        self.give_up_on_a_silent_host(now);
        self.send_path_info(now);
        self.share_game_readiness();
        self.send_seat_parts();
    }

    fn new_session(&mut self, role: Role) -> Result<RemoteSession, NetError> {
        let sink = self.progress.fresh_sink();
        self.progress_logged = 0;
        self.session_asked_at = Some(Instant::now());
        let transport = (self.factory)().map_err(|error| NetError::Failed(error.to_string()))?;
        let mut session = Session::new(
            transport,
            SessionConfig {
                role,
                name: self.config.name.clone(),
                build: self.config.build,
                rules: self.rules,
            },
        );
        session.report_progress(sink);
        Ok(session)
    }

    fn log_progress(&mut self) {
        let Some(asked_at) = self.session_asked_at else {
            return;
        };
        let entries = self.progress.entries();
        for entry in entries.iter().skip(self.progress_logged) {
            self.emit(LauncherEvent::LinkProgress {
                after: entry.at.saturating_duration_since(asked_at),
                line: entry.progress.describe(),
            });
        }
        self.progress_logged = entries.len();
    }

    fn give_up_on_a_silent_host(&mut self, now: Instant) {
        let joining = self.status().state == ConnectionState::Joining;
        let Some(asked_at) = self.session_asked_at.filter(|_| joining) else {
            return;
        };
        let entries = self.progress.entries();
        let key_not_found = entries
            .iter()
            .any(|entry| matches!(entry.progress, LinkProgress::HostNotFound { .. }));
        let dial_failed = entries
            .iter()
            .any(|entry| entry.progress == LinkProgress::DialFailed);
        let timed_out = now.saturating_duration_since(asked_at) >= HOST_ANSWER_TIMEOUT;
        if !key_not_found && !dial_failed && !timed_out {
            return;
        }
        let message = if key_not_found {
            HOST_KEY_NOT_FOUND_TEXT
        } else {
            HOST_UNREACHABLE_TEXT
        };
        self.end_session(SessionEndCause::HostUnreachable);
        self.status().state = ConnectionState::Failed(message.to_owned());
        self.emit(LauncherEvent::Failed {
            message: message.to_owned(),
        });
    }

    fn pump_link(&mut self) {
        let mut incoming = Vec::new();
        self.link.poll(&mut incoming);
        for event in incoming {
            match event {
                TransportEvent::PeerConnected(_) => {
                    self.local_game.game_linked();
                    self.status().game_linked = true;
                    self.emit(LauncherEvent::GameLinked);
                    self.welcome_game();
                    if let Some(settings) = self.settings {
                        self.send_to_game(&Message::Settings(settings));
                    }
                }
                TransportEvent::PeerDisconnected(_, reason) => {
                    self.local_game.game_unlinked();
                    self.status().game_linked = false;
                    let detail = format!(
                        "link {}; {}",
                        reason.label(),
                        self.traffic.describe(Instant::now())
                    );
                    self.emit(LauncherEvent::GameUnlinked { detail });
                }
                TransportEvent::Message { payload, .. } => self.on_game_payload(&payload),
            }
        }
    }

    fn on_game_payload(&mut self, payload: &[u8]) {
        let (envelope, message) = match decode_enveloped(payload) {
            Ok(framed) => framed,
            Err(error) => {
                self.report_refused("undecodable game message", &error);
                return;
            }
        };
        match message {
            Message::Readiness(readiness) => {
                self.local_game.game_reported(readiness);
                return;
            }
            Message::Settings(settings) => {
                self.settings = Some(settings);
                self.emit(LauncherEvent::GameChangedSettings { settings });
                return;
            }
            Message::SessionOptions(options) => {
                self.accept_options_from_game(options);
                return;
            }
            Message::GameSaved { succeeded } => {
                self.emit(LauncherEvent::GameSaved { succeeded });
                return;
            }
            Message::LobbyReady(_)
            | Message::LobbyStart(_)
            | Message::LobbyBody(_)
            | Message::CampaignCard(_)
            | Message::SeatPart(_)
            | Message::SeatReceipt(_)
            | Message::QuitGame => return,
            _ => {}
        }
        if !self.members.addresses_a_member(envelope.to) {
            return;
        }
        let addressed = envelope.to.filter(|_| self.members.count() > 1);
        let Some(session) = self.remote.as_mut() else {
            return;
        };
        match message {
            state @ Message::State(_) => {
                let sent = match addressed {
                    Some(slot) => session.send_ordered_to(slot, &state),
                    None => session.send_ordered(&state),
                };
                if let Err(error) = sent {
                    self.report_refused("State", &error);
                }
                self.traffic.note_sent("State", Instant::now());
            }
            event if event.is_relayed() => {
                let sent = match addressed {
                    Some(slot) => session.send_relayed_to(slot, &event),
                    None => session.send_relayed(&event),
                };
                if let Err(error) = sent {
                    self.report_refused(event.kind_name(), &error);
                }
                self.traffic.note_sent(event.kind_name(), Instant::now());
            }
            _ => {}
        }
    }

    fn accept_options_from_game(&mut self, options: SessionOptions) {
        if self.status().guest {
            return;
        }
        self.options = options;
        if options_differ_from_last(&mut self.last_options_announced, options) {
            self.emit(LauncherEvent::SessionOptionsChanged { options });
        }
        self.send_options_if_hosting();
    }

    fn pump_remote(&mut self, now: Instant) {
        let Some(session) = self.remote.as_mut() else {
            return;
        };
        let mut events = Vec::new();
        session.poll(now, &mut events);
        for event in events {
            self.on_remote_event(event);
        }
    }

    fn on_remote_event(&mut self, event: SessionEvent<Message>) {
        if let SessionEvent::Message { from, .. } = &event
            && !self.members.is_member(*from)
        {
            return;
        }
        match event {
            SessionEvent::PlayerJoined { slot, name, peer } => {
                let first_member = self.members.is_empty();
                self.members.admit(slot, name.clone(), peer);
                self.traffic.forget();
                self.readiness_sent = None;
                self.send_own_lobby();
                self.send_own_body();
                self.send_own_card();
                let mut status = self.status();
                status.state = ConnectionState::Connected;
                self.members.write_status(&mut status);
                if let Some(session) = self.remote.as_ref() {
                    status.rules = session.rules();
                    status.own_slot = session.local_slot();
                }
                drop(status);
                self.emit(LauncherEvent::MemberJoined { slot, name });
                if first_member {
                    self.welcome_game();
                } else if let Some(announcement) = self.members.member_announcement(slot) {
                    self.send_to_game(&announcement);
                }
            }
            SessionEvent::Message {
                from,
                message: message @ Message::State(_),
            } => {
                self.traffic.note_received("State", Instant::now());
                self.relay_to_game(from, &message);
            }
            SessionEvent::Message { from, message } => {
                self.traffic
                    .note_received(message.kind_name(), Instant::now());
                match message {
                    Message::Readiness(readiness) => {
                        self.change_member(from, MemberChange::Readiness(readiness));
                    }
                    Message::SessionOptions(options) => {
                        if options_differ_from_last(&mut self.last_options_announced, options) {
                            self.emit(LauncherEvent::SessionOptionsChanged { options });
                        }
                    }
                    Message::LobbyReady(ready) => {
                        self.change_member(from, MemberChange::Lobby(ready));
                        return;
                    }
                    Message::LobbyBody(LobbyBody { body }) => {
                        self.change_member(from, MemberChange::Body(body));
                        return;
                    }
                    Message::LobbyStart(start) => {
                        if self.status().guest {
                            self.emit(LauncherEvent::LobbyStartReceived { mode: start.mode });
                        }
                        return;
                    }
                    Message::CampaignCard(card) => {
                        if self.status().guest && from == 0 {
                            self.emit(LauncherEvent::CampaignCardReceived { card });
                        }
                        return;
                    }
                    Message::SeatPart(part) => {
                        self.on_seat_part(from, &part);
                        return;
                    }
                    Message::SeatReceipt(receipt) => {
                        self.on_seat_receipt(from, receipt);
                        return;
                    }
                    Message::GameSaved { .. } => return,
                    _ => {}
                }
                self.relay_to_game(from, &message);
            }
            SessionEvent::PlayerLeft { slot, reason, .. } => self.on_peer_left(slot, reason),
            SessionEvent::Rejected { reason } => {
                let last_seen = self.status().clone();
                self.remote = None;
                self.forget_peer();
                self.last_reject = Some(reason);
                let mut status = self.status();
                status.state = ConnectionState::Rejected(reason);
                drop(status);
                self.send_to_game(&Message::Reject(replicant_proto::Reject { reason }));
                self.emit(LauncherEvent::Rejected { reason });
                self.emit(LauncherEvent::SessionEnded {
                    cause: SessionEndCause::Refused(reason),
                    last_seen,
                });
            }
            SessionEvent::RulesChanged(rules) => {
                self.status().rules = rules;
                self.send_to_game(&Message::Rules(rules));
            }
            SessionEvent::MemberPings(pings) => {
                let mut status = self.status();
                for ping in pings {
                    if let Some(member) = status.members.iter_mut().find(|m| m.slot == ping.slot) {
                        member.rtt_ms = Some(ping.rtt_ms);
                    }
                }
            }
            SessionEvent::PeerRefused { reason, .. } => {
                self.emit(LauncherEvent::PeerRefused { reason });
            }
        }
    }

    fn on_peer_left(&mut self, slot: u8, reason: LeaveReason) {
        let last_seen = self.status().clone();
        if self.members.leave(slot).is_none() {
            return;
        }
        self.incoming_seats.remove(&slot);
        self.outgoing_seats.remove(&slot);
        let hosting = self
            .remote
            .as_ref()
            .is_some_and(|session| session.local_slot() == Some(0));
        let host_left = !hosting && slot == 0;
        if host_left {
            self.members.clear();
        }
        let others_remain = !self.members.is_empty();
        if !others_remain {
            self.forget_peer();
        }
        if host_left {
            self.send_to_game(&Message::Bye);
        } else {
            self.send_to_game(&Message::MemberLeft { slot });
        }
        let detail = format!(
            "{}; {}",
            reason.label(),
            self.traffic.describe(Instant::now())
        );
        self.emit(LauncherEvent::MemberLeft { slot, detail });
        if others_remain {
            let mut status = self.status();
            self.members.write_status(&mut status);
            return;
        }
        if hosting {
            self.status().state = ConnectionState::Hosting;
            return;
        }
        self.remote = None;
        self.status().own_slot = None;
        let cause = SessionEndCause::from_host_leaving(reason);
        let message = cause.reason_line().unwrap_or_default();
        self.status().state = ConnectionState::Failed(message.to_owned());
        self.emit(LauncherEvent::SessionEnded { cause, last_seen });
    }

    fn forget_peer(&mut self) {
        self.members.clear();
        self.incoming_seats.clear();
        self.outgoing_seats.clear();
        self.last_path_info = None;
        self.readiness_sent = None;
        self.last_options_announced = None;
        let mut status = self.status();
        self.members.write_status(&mut status);
    }

    fn change_member(&mut self, slot: u8, change: MemberChange) {
        let mut status = self.status();
        if let Some(member) = status.members.iter_mut().find(|row| row.slot == slot) {
            member.apply(change);
        }
        drop(status);
        self.emit(LauncherEvent::MemberChanged { slot, change });
    }

    fn welcome_game(&mut self) {
        let Some(first) = self.members.first().cloned() else {
            if let Some(reason) = self.last_reject {
                self.send_to_game(&Message::Reject(replicant_proto::Reject { reason }));
            }
            return;
        };
        self.send_to_game(&Message::Welcome(Welcome {
            version: PROTOCOL_VERSION,
            slot: self.own_slot(),
            host_name: first.name.clone(),
            rules: self.remote.as_ref().map_or(self.rules, Session::rules),
        }));
        let slots: Vec<u8> = self.members.all().map(|member| member.slot).collect();
        for slot in slots {
            if let Some(announcement) = self.members.member_announcement(slot) {
                self.send_to_game(&announcement);
            }
        }
        let first_readiness = self
            .status()
            .member(first.slot)
            .map_or(GameReadiness::NotLinked, |member| member.readiness);
        self.relay_to_game(first.slot, &Message::Readiness(first_readiness));
    }

    fn send_own_lobby(&mut self) {
        if self.members.is_empty() {
            return;
        }
        let (Some(ready), Some(session)) = (self.own_lobby, self.remote.as_mut()) else {
            return;
        };
        let _ = session.send_relayed(&Message::LobbyReady(ready));
    }

    fn send_own_body(&mut self) {
        if self.members.is_empty() {
            return;
        }
        let (Some(body), Some(session)) = (self.own_body, self.remote.as_mut()) else {
            return;
        };
        let _ = session.send_relayed(&Message::LobbyBody(LobbyBody { body }));
    }

    fn send_own_card(&mut self) {
        if self.members.is_empty() {
            return;
        }
        let (Some(card), Some(session)) = (self.own_card.clone(), self.remote.as_mut()) else {
            return;
        };
        let _ = session.send_relayed(&Message::CampaignCard(card));
    }

    fn send_seat_parts(&mut self) {
        let Some(session) = self.remote.as_mut() else {
            return;
        };
        let mut events = Vec::new();
        for (slot, sender) in &mut self.outgoing_seats {
            let batch = sender.next_parts(SEAT_PARTS_PER_PUMP);
            if batch.is_empty() {
                continue;
            }
            for part in batch {
                let _ = session.send_relayed_to(*slot, &Message::SeatPart(part));
            }
            events.push(LauncherEvent::SeatProgress {
                slot: *slot,
                transfer: sender.transfer(),
                parts_done: sender.parts_sent(),
                parts: sender.parts(),
                incoming: false,
            });
        }
        for event in events {
            self.emit(event);
        }
    }

    fn on_seat_part(&mut self, from: u8, part: &SeatPart) {
        let step = self.incoming_seats.entry(from).or_default().accept(part);
        let outcome = match step {
            AssemblerStep::Progress { parts_done, parts } => {
                self.emit(LauncherEvent::SeatProgress {
                    slot: from,
                    transfer: part.transfer,
                    parts_done,
                    parts,
                    incoming: true,
                });
                return;
            }
            AssemblerStep::Ignored => return,
            AssemblerStep::Whole(seat) => {
                self.emit(LauncherEvent::SeatReceived {
                    slot: from,
                    transfer: part.transfer,
                    seat,
                });
                SeatOutcome::ReceivedWholeAndValid
            }
            AssemblerStep::Refused(reason) => {
                self.emit(LauncherEvent::SeatTransferRefused {
                    slot: from,
                    transfer: part.transfer,
                    reason,
                    by_peer: false,
                });
                SeatOutcome::Refused(reason)
            }
        };
        let receipt = Message::SeatReceipt(SeatReceipt {
            transfer: part.transfer,
            outcome,
        });
        if let Some(session) = self.remote.as_mut() {
            let _ = session.send_relayed_to(from, &receipt);
        }
    }

    fn on_seat_receipt(&mut self, from: u8, receipt: SeatReceipt) {
        let answers_current = self
            .outgoing_seats
            .get(&from)
            .is_some_and(|sender| sender.transfer() == receipt.transfer);
        if !answers_current {
            return;
        }
        self.outgoing_seats.remove(&from);
        self.emit(match receipt.outcome {
            SeatOutcome::ReceivedWholeAndValid => LauncherEvent::SeatDelivered {
                slot: from,
                transfer: receipt.transfer,
            },
            SeatOutcome::Refused(reason) => LauncherEvent::SeatTransferRefused {
                slot: from,
                transfer: receipt.transfer,
                reason,
                by_peer: true,
            },
        });
    }

    fn share_game_readiness(&mut self) {
        let current = self.local_game.current();
        if self.own_readiness_logged != Some(current) {
            self.own_readiness_logged = Some(current);
            self.emit(LauncherEvent::OwnGameReadiness { readiness: current });
        }
        if self.readiness_sent == Some(current) {
            return;
        }
        if self.members.is_empty() {
            return;
        }
        let Some(session) = self.remote.as_mut() else {
            return;
        };
        if session.send_relayed(&Message::Readiness(current)).is_ok() {
            self.readiness_sent = Some(current);
        }
    }

    fn send_path_info(&mut self, now: Instant) {
        let due = self
            .last_path_info
            .is_none_or(|at| now.saturating_duration_since(at) >= PATH_INFO_INTERVAL);
        let (Some(peer), Some(session)) = (
            self.members.first().map(|member| member.peer),
            self.remote.as_ref(),
        ) else {
            return;
        };
        if !due {
            return;
        }
        let rtt = session.rtt(peer);
        let direct = session
            .transport()
            .link_stats(peer)
            .map(|stats| stats.direct);
        self.last_path_info = Some(now);
        self.measure_member_pings();
        self.send_options_if_hosting();
        let mut status = self.status();
        status.rtt = rtt;
        status.direct_path = direct;
        drop(status);
        if let (Some(rtt), Some(direct)) = (rtt, direct) {
            let rtt_micros = u32::try_from(rtt.as_micros()).unwrap_or(u32::MAX);
            self.send_to_game(&Message::PathInfo(PathInfo { rtt_micros, direct }));
        }
    }

    fn measure_member_pings(&mut self) {
        let hosting = self
            .remote
            .as_ref()
            .is_some_and(|session| session.local_slot() == Some(0));
        let Some(session) = self.remote.as_ref().filter(|_| hosting) else {
            return;
        };
        let measured: Vec<(u8, Option<u32>)> = self
            .members
            .all()
            .map(|member| {
                let rtt_ms = session
                    .rtt(member.peer)
                    .map(|rtt| u32::try_from(rtt.as_millis()).unwrap_or(u32::MAX));
                (member.slot, rtt_ms)
            })
            .collect();
        let mut status = self.status();
        for (slot, rtt_ms) in measured {
            if let Some(member) = status.members.iter_mut().find(|row| row.slot == slot) {
                member.rtt_ms = rtt_ms.or(member.rtt_ms);
            }
        }
    }

    fn send_options_if_hosting(&mut self) {
        let hosting = self
            .remote
            .as_ref()
            .is_some_and(|session| session.local_slot() == Some(0));
        if !hosting || self.members.is_empty() {
            return;
        }
        let message = Message::SessionOptions(self.options);
        self.send_to_game(&message);
        if let Some(session) = self.remote.as_mut() {
            let _ = session.send_relayed(&message);
        }
    }

    fn report_refused(&mut self, kind: &'static str, error: &WireError) {
        if self.refusals.should_log(kind, Instant::now()) {
            self.emit(LauncherEvent::MessageRefused {
                kind: kind.to_owned(),
                reason: error.to_string(),
            });
        }
    }

    fn own_slot(&self) -> u8 {
        self.remote
            .as_ref()
            .and_then(Session::local_slot)
            .unwrap_or(0)
    }

    fn send_to_game(&mut self, message: &Message) {
        self.relay_to_game(self.own_slot(), message);
    }

    fn relay_to_game(&mut self, from: u8, message: &Message) {
        let envelope = Envelope { from, to: None };
        let bytes = match encode_enveloped(&envelope, message) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.report_refused(message.kind_name(), &error);
                return;
            }
        };
        let channel = if message.is_latest_wins() {
            Channel::UnreliableLatest
        } else {
            Channel::ReliableOrdered
        };
        let _ = self.link.send(LINK_GAME_PEER, channel, &bytes);
    }

    fn emit(&self, event: LauncherEvent) {
        let _ = self.events.send(event);
    }

    fn status(&self) -> MutexGuard<'_, LinkStatus> {
        self.status.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
