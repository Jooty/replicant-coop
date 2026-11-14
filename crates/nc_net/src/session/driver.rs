use std::collections::HashMap;
use std::marker::PhantomData;
use std::time::{Duration, Instant};

use nc_wire::{
    Envelope, GameMessage, Hello, LinkPayload, Member, MemberPing, Ping, Pong, Reject,
    RejectReason, SessionRules, Welcome, WireError, decode, decode_enveloped, encode,
    encode_enveloped,
};

use crate::progress::{LinkProgress, ProgressSink};
use crate::session::peer::{PeerSession, Phase};
use crate::session::{
    HANDSHAKE_TIMEOUT, KEEPALIVE_INTERVAL, LeaveReason, MEMBER_PING_LIST_INTERVAL, PEER_TIMEOUT,
    REFUSAL_LINGER, Role, SessionConfig, SessionEvent,
};
use crate::transport::{
    Channel, DisconnectReason, Invite, NetError, PeerId, Transport, TransportEvent,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PingCounts {
    pub sent: u32,
    pub answered: u32,
    pub awaiting_answer: bool,
}

pub struct Session<T: Transport, M: GameMessage> {
    transport: T,
    config: SessionConfig,
    peers: HashMap<PeerId, PeerSession>,
    next_nonce: u32,
    local_slot: Option<u8>,
    rules: SessionRules,
    refused_peer_version: Option<u16>,
    next_ping_list_at: Option<Instant>,
    last_now: Instant,
    pending_events: Vec<SessionEvent<M>>,
    progress: Option<ProgressSink>,
    message: PhantomData<fn() -> M>,
}

impl<T: Transport, M: GameMessage> Session<T, M> {
    pub fn new(transport: T, config: SessionConfig) -> Self {
        let local_slot = (config.role == Role::Host).then_some(0);
        let rules = clamped_rules::<M>(config.rules);
        Self {
            transport,
            config,
            peers: HashMap::new(),
            next_nonce: 0,
            local_slot,
            rules,
            refused_peer_version: None,
            next_ping_list_at: None,
            last_now: Instant::now(),
            pending_events: Vec::new(),
            progress: None,
            message: PhantomData,
        }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn rules(&self) -> SessionRules {
        self.rules
    }

    pub fn set_rules(&mut self, rules: SessionRules) {
        if self.config.role != Role::Host {
            return;
        }
        let rules = clamped_rules::<M>(rules);
        self.rules = rules;
        for peer in self.ready_peers() {
            self.send_link(peer, LinkPayload::Rules(rules));
        }
    }

    /// The name this side gives every player who joins from now on; players already in keep the one they
    /// were told.
    pub fn set_name(&mut self, name: &str) {
        self.config.name = name.to_owned();
    }

    pub fn local_slot(&self) -> Option<u8> {
        self.local_slot
    }

    /// The protocol version the last player refused for a version mismatch asked with.
    pub fn refused_peer_version(&self) -> Option<u16> {
        self.refused_peer_version
    }

    pub fn host(&mut self) -> Result<Invite, NetError> {
        self.transport.host()
    }

    /// Sends what the link and the handshake do from now on to `sink`.
    pub fn report_progress(&mut self, sink: ProgressSink) {
        self.transport.report_progress(sink.clone());
        self.progress = Some(sink);
    }

    pub fn join(&mut self, invite: &Invite) -> Result<(), NetError> {
        self.transport.join(invite)
    }

    pub fn rtt(&self, peer: PeerId) -> Option<Duration> {
        self.peers.get(&peer)?.rtt
    }

    pub fn ping_counts(&self, peer: PeerId) -> Option<PingCounts> {
        let session = self.peers.get(&peer)?;
        Some(PingCounts {
            sent: session.pings_sent,
            answered: session.pongs_received,
            awaiting_answer: session.outstanding_ping.is_some(),
        })
    }

    pub fn ready_peers(&self) -> Vec<PeerId> {
        self.peers
            .iter()
            .filter(|(_, session)| session.phase == Phase::Ready)
            .map(|(peer, _)| *peer)
            .collect()
    }

    pub fn send_ordered(&mut self, message: &M) -> Result<(), WireError> {
        if message.ordered_tick().is_none() {
            return Err(WireError::OutOfRange { field: "ordered" });
        }
        message.validate()?;
        for peer in self.ready_peers() {
            self.send_to(peer, message);
        }
        Ok(())
    }

    pub fn send_relayed(&mut self, message: &M) -> Result<(), WireError> {
        if !message.is_relayed() {
            return Err(WireError::OutOfRange { field: "relayed" });
        }
        message.validate()?;
        for peer in self.ready_peers() {
            self.send_to(peer, message);
        }
        Ok(())
    }

    pub fn send_ordered_to(&mut self, slot: u8, message: &M) -> Result<(), WireError> {
        if message.ordered_tick().is_none() {
            return Err(WireError::OutOfRange { field: "ordered" });
        }
        message.validate()?;
        self.send_addressed(slot, message);
        Ok(())
    }

    pub fn send_relayed_to(&mut self, slot: u8, message: &M) -> Result<(), WireError> {
        if !message.is_relayed() {
            return Err(WireError::OutOfRange { field: "relayed" });
        }
        message.validate()?;
        self.send_addressed(slot, message);
        Ok(())
    }

    pub fn close(&mut self) {
        for peer in self.ready_peers() {
            self.send_link(peer, LinkPayload::Bye);
        }
    }

    pub fn remove(&mut self, slot: u8) -> bool {
        if self.config.role != Role::Host || slot == 0 {
            return false;
        }
        let Some(peer) = self.ready_peer_in_slot(slot) else {
            return false;
        };
        let reject = Reject {
            reason: RejectReason::RemovedByHost,
        };
        self.send_link(peer, LinkPayload::Reject(reject));
        if let Some(session) = self.peers.get_mut(&peer) {
            session.phase = Phase::Refused {
                drop_at: self.last_now + REFUSAL_LINGER,
            };
        }
        for other in self.ready_peers() {
            self.send_link(other, LinkPayload::MemberLeft { slot });
        }
        self.pending_events.push(SessionEvent::PlayerLeft {
            slot,
            reason: LeaveReason::RemovedByHost,
            peer,
        });
        true
    }

    pub fn poll(&mut self, now: Instant, out: &mut Vec<SessionEvent<M>>) {
        self.last_now = now;
        out.append(&mut self.pending_events);
        let mut incoming = Vec::new();
        self.transport.poll(&mut incoming);
        for event in incoming {
            match event {
                TransportEvent::PeerConnected(peer) => self.on_connected(peer, now),
                TransportEvent::PeerDisconnected(peer, reason) => {
                    self.on_disconnected(peer, reason, out);
                }
                TransportEvent::Message { from, payload, .. } => {
                    self.on_payload(from, &payload, now, out);
                }
            }
        }
        self.run_timers(now, out);
    }

    fn on_connected(&mut self, peer: PeerId, now: Instant) {
        self.peers.insert(peer, PeerSession::new(now));
        if self.config.role == Role::Guest {
            let hello = LinkPayload::Hello(Hello {
                version: M::PROTOCOL_VERSION,
                build: self.config.build,
                name: self.config.name.clone(),
            });
            self.send_bare(peer, hello);
            self.tell_progress(LinkProgress::HelloSent {
                protocol: M::PROTOCOL_VERSION,
            });
        }
    }

    fn on_disconnected(
        &mut self,
        peer: PeerId,
        reason: DisconnectReason,
        out: &mut Vec<SessionEvent<M>>,
    ) {
        let leave = match reason {
            DisconnectReason::TimedOut => LeaveReason::TimedOut,
            DisconnectReason::Closed | DisconnectReason::Failed => LeaveReason::TransportClosed,
        };
        self.forget_peer(peer, leave, out);
    }

    fn on_payload(
        &mut self,
        peer: PeerId,
        payload: &[u8],
        now: Instant,
        out: &mut Vec<SessionEvent<M>>,
    ) {
        let Some(session) = self.peers.get_mut(&peer) else {
            return;
        };
        let phase = session.phase;
        if matches!(phase, Phase::Refused { .. }) {
            return;
        }
        session.last_heard = now;
        let peer_slot = session.slot;
        if phase == Phase::Ready {
            let Ok((envelope, message)) = decode_enveloped::<M>(payload) else {
                self.drop_peer(peer, LeaveReason::ProtocolViolation, out);
                return;
            };
            if let Some(to) = envelope.to
                && Some(to) != self.local_slot
            {
                if self.config.role == Role::Host {
                    self.forward_addressed(peer, peer_slot, to, message, out);
                }
                return;
            }
            // The host writes the sender itself, so a guest cannot speak for another slot.
            let from = match self.config.role {
                Role::Host => peer_slot,
                Role::Guest => envelope.from,
            };
            if self.config.role == Role::Guest && !self.is_possible_sender(from) {
                return;
            }
            self.on_ready_message(peer, from, message, now, out);
            return;
        }
        let Ok(message) = decode::<M>(payload) else {
            self.drop_peer(peer, LeaveReason::ProtocolViolation, out);
            return;
        };
        match (phase, self.config.role, message.into_link()) {
            (Phase::Handshaking, Role::Host, Ok(LinkPayload::Hello(hello))) => {
                self.on_hello(peer, &hello, now, out);
            }
            (Phase::Handshaking, Role::Guest, Ok(LinkPayload::Welcome(welcome))) => {
                self.on_welcome(peer, &welcome, out);
            }
            (Phase::Handshaking, Role::Guest, Ok(LinkPayload::Reject(reject))) => {
                self.peers.remove(&peer);
                self.transport.disconnect(peer);
                out.push(SessionEvent::Rejected {
                    reason: reject.reason,
                });
            }
            _ => self.drop_peer(peer, LeaveReason::ProtocolViolation, out),
        }
    }

    fn on_hello(
        &mut self,
        peer: PeerId,
        hello: &Hello,
        now: Instant,
        out: &mut Vec<SessionEvent<M>>,
    ) {
        let refusal = if hello.version != M::PROTOCOL_VERSION {
            self.refused_peer_version = Some(hello.version);
            Some(RejectReason::VersionMismatch)
        } else if hello.build != self.config.build {
            Some(RejectReason::BuildMismatch)
        } else if self.free_slot().is_none() {
            Some(RejectReason::SessionFull)
        } else {
            None
        };
        if let Some(reason) = refusal {
            self.refuse(peer, reason, now, out);
            return;
        }
        self.admit(peer, &hello.name, out);
    }

    fn refuse(
        &mut self,
        peer: PeerId,
        reason: RejectReason,
        now: Instant,
        out: &mut Vec<SessionEvent<M>>,
    ) {
        self.send_bare(peer, LinkPayload::Reject(Reject { reason }));
        if let Some(session) = self.peers.get_mut(&peer) {
            session.phase = Phase::Refused {
                drop_at: now + REFUSAL_LINGER,
            };
        }
        out.push(SessionEvent::PeerRefused { peer, reason });
    }

    fn admit(&mut self, peer: PeerId, name: &str, out: &mut Vec<SessionEvent<M>>) {
        let Some(slot) = self.free_slot() else {
            return;
        };
        if let Some(session) = self.peers.get_mut(&peer) {
            session.phase = Phase::Ready;
            session.slot = slot;
        }
        let welcome = LinkPayload::Welcome(Welcome {
            version: M::PROTOCOL_VERSION,
            slot,
            host_name: self.config.name.clone(),
            rules: self.rules,
        });
        self.send_bare(peer, welcome);
        self.announce_members(peer, slot, name);
        out.push(SessionEvent::PlayerJoined {
            slot,
            name: name.to_owned(),
            peer,
        });
    }

    fn announce_members(&mut self, joined: PeerId, slot: u8, name: &str) {
        let mut others: Vec<(PeerId, u8, String)> = self
            .peers
            .iter()
            .filter(|(peer, session)| **peer != joined && session.phase == Phase::Ready)
            .map(|(peer, session)| (*peer, session.slot, session.name.clone()))
            .collect();
        others.sort_by_key(|(_, other_slot, _)| *other_slot);
        if let Some(session) = self.peers.get_mut(&joined) {
            session.name = name.to_owned();
        }
        for (peer, other_slot, other_name) in others {
            self.send_link(
                joined,
                LinkPayload::Member(Member {
                    slot: other_slot,
                    name: other_name,
                }),
            );
            self.send_link(
                peer,
                LinkPayload::Member(Member {
                    slot,
                    name: name.to_owned(),
                }),
            );
        }
    }

    fn is_possible_sender(&self, from: u8) -> bool {
        Some(from) != self.local_slot && from < M::MAX_PLAYERS
    }

    fn is_announceable_guest_slot(&self, slot: u8) -> bool {
        slot != 0 && self.is_possible_sender(slot)
    }

    fn on_welcome(&mut self, peer: PeerId, welcome: &Welcome, out: &mut Vec<SessionEvent<M>>) {
        if welcome.version != M::PROTOCOL_VERSION {
            self.peers.remove(&peer);
            self.transport.disconnect(peer);
            out.push(SessionEvent::Rejected {
                reason: RejectReason::VersionMismatch,
            });
            return;
        }
        if let Some(session) = self.peers.get_mut(&peer) {
            session.phase = Phase::Ready;
            session.slot = 0;
        }
        self.local_slot = Some(welcome.slot);
        self.rules = welcome.rules;
        self.tell_progress(LinkProgress::WelcomeRead {
            seat: welcome.slot,
            host_name: welcome.host_name.clone(),
        });
        out.push(SessionEvent::PlayerJoined {
            slot: 0,
            name: welcome.host_name.clone(),
            peer,
        });
    }

    fn on_ready_message(
        &mut self,
        peer: PeerId,
        from: u8,
        message: M,
        now: Instant,
        out: &mut Vec<SessionEvent<M>>,
    ) {
        let message = match message.into_link() {
            Ok(link) => {
                self.on_ready_link(peer, link, now, out);
                return;
            }
            Err(message) => message,
        };
        match self.accept_game_message(peer, from, &message) {
            Some(true) => {
                if self.config.role == Role::Host && message.forwarded_to_other_guests() {
                    self.send_from_to_other_guests(peer, from, &message);
                }
                out.push(SessionEvent::Message { from, message });
            }
            Some(false) => {}
            None => self.drop_peer(peer, LeaveReason::ProtocolViolation, out),
        }
    }

    fn accept_game_message(&mut self, peer: PeerId, from: u8, message: &M) -> Option<bool> {
        if let Some(tick) = message.ordered_tick() {
            Some(
                self.peers
                    .get_mut(&peer)
                    .is_some_and(|session| session.accepts_tick(from, tick)),
            )
        } else {
            message.is_relayed().then_some(true)
        }
    }

    fn forward_addressed(
        &mut self,
        peer: PeerId,
        from: u8,
        to: u8,
        message: M,
        out: &mut Vec<SessionEvent<M>>,
    ) {
        if message.clone().into_link().is_ok() {
            self.drop_peer(peer, LeaveReason::ProtocolViolation, out);
            return;
        }
        match self.accept_game_message(peer, from, &message) {
            Some(true) => {}
            Some(false) => return,
            None => {
                self.drop_peer(peer, LeaveReason::ProtocolViolation, out);
                return;
            }
        }
        if to == from {
            return;
        }
        if let Some(target) = self.ready_peer_in_slot(to) {
            let envelope = Envelope { from, to: Some(to) };
            self.send_enveloped(target, &envelope, &message);
        }
    }

    fn send_from_to_other_guests(&mut self, sender: PeerId, from: u8, message: &M) {
        let envelope = Envelope { from, to: None };
        for target in self.ready_peers() {
            if target != sender {
                self.send_enveloped(target, &envelope, message);
            }
        }
    }

    fn ready_peer_in_slot(&self, slot: u8) -> Option<PeerId> {
        self.peers
            .iter()
            .find(|(_, session)| session.phase == Phase::Ready && session.slot == slot)
            .map(|(peer, _)| *peer)
    }

    fn on_ready_link(
        &mut self,
        peer: PeerId,
        link: LinkPayload,
        now: Instant,
        out: &mut Vec<SessionEvent<M>>,
    ) {
        match link {
            LinkPayload::Ping(Ping { nonce }) => {
                self.send_link(peer, LinkPayload::Pong(Pong { nonce }));
            }
            LinkPayload::Pong(Pong { nonce }) => {
                if let Some(session) = self.peers.get_mut(&peer)
                    && let Some((expected, sent_at)) = session.outstanding_ping
                    && expected == nonce
                {
                    session.rtt = Some(now.saturating_duration_since(sent_at));
                    session.pongs_received += 1;
                    session.outstanding_ping = None;
                }
            }
            LinkPayload::Bye => self.drop_peer(peer, LeaveReason::Quit, out),
            LinkPayload::Reject(Reject {
                reason: RejectReason::RemovedByHost,
            }) if self.config.role == Role::Guest => {
                self.drop_peer(peer, LeaveReason::RemovedByHost, out);
            }
            LinkPayload::MemberPings(pings) if self.config.role == Role::Guest => {
                out.push(SessionEvent::MemberPings(pings));
            }
            LinkPayload::Rules(rules) if self.config.role == Role::Guest => {
                self.rules = rules;
                out.push(SessionEvent::RulesChanged(rules));
            }
            LinkPayload::Member(member)
                if self.config.role == Role::Guest
                    && self.is_announceable_guest_slot(member.slot) =>
            {
                out.push(SessionEvent::PlayerJoined {
                    slot: member.slot,
                    name: member.name,
                    peer,
                });
            }
            LinkPayload::MemberLeft { slot }
                if self.config.role == Role::Guest && self.is_announceable_guest_slot(slot) =>
            {
                out.push(SessionEvent::PlayerLeft {
                    slot,
                    reason: LeaveReason::Quit,
                    peer,
                });
            }
            LinkPayload::Hello(_)
            | LinkPayload::Welcome(_)
            | LinkPayload::Reject(_)
            | LinkPayload::Member(_)
            | LinkPayload::MemberPings(_)
            | LinkPayload::MemberLeft { .. }
            | LinkPayload::Rules(_) => {
                self.drop_peer(peer, LeaveReason::ProtocolViolation, out);
            }
        }
    }

    fn run_timers(&mut self, now: Instant, out: &mut Vec<SessionEvent<M>>) {
        if self.config.role == Role::Host {
            self.share_member_pings(now);
        }
        let peers: Vec<PeerId> = self.peers.keys().copied().collect();
        for peer in peers {
            let Some(session) = self.peers.get(&peer) else {
                continue;
            };
            let phase = session.phase;
            let silent_for = now.saturating_duration_since(session.last_heard);
            let handshaking_for = now.saturating_duration_since(session.connected_at);
            let since_ping = now.saturating_duration_since(session.last_ping_sent);
            match phase {
                Phase::Refused { drop_at } if now >= drop_at => {
                    self.drop_peer(peer, LeaveReason::ProtocolViolation, out);
                }
                Phase::Refused { .. } => {}
                Phase::Handshaking if handshaking_for > HANDSHAKE_TIMEOUT => {
                    self.drop_peer(peer, LeaveReason::TimedOut, out);
                }
                Phase::Handshaking => {}
                Phase::Ready if silent_for > PEER_TIMEOUT => {
                    self.drop_peer(peer, LeaveReason::TimedOut, out);
                }
                Phase::Ready if since_ping >= KEEPALIVE_INTERVAL => self.ping(peer, now),
                Phase::Ready => {}
            }
        }
    }

    fn share_member_pings(&mut self, now: Instant) {
        let due_at = *self
            .next_ping_list_at
            .get_or_insert(now + MEMBER_PING_LIST_INTERVAL);
        if now < due_at {
            return;
        }
        self.next_ping_list_at = Some(now + MEMBER_PING_LIST_INTERVAL);
        let mut pings = vec![MemberPing { slot: 0, rtt_ms: 0 }];
        pings.extend(
            self.peers
                .values()
                .filter(|session| session.phase == Phase::Ready)
                .filter_map(|session| {
                    let rtt = session.rtt?;
                    Some(MemberPing {
                        slot: session.slot,
                        rtt_ms: u32::try_from(rtt.as_millis()).unwrap_or(u32::MAX),
                    })
                }),
        );
        pings.sort_by_key(|ping| ping.slot);
        for peer in self.ready_peers() {
            self.send_link(peer, LinkPayload::MemberPings(pings.clone()));
        }
    }

    fn ping(&mut self, peer: PeerId, now: Instant) {
        self.next_nonce = self.next_nonce.wrapping_add(1);
        let nonce = self.next_nonce;
        if let Some(session) = self.peers.get_mut(&peer) {
            session.last_ping_sent = now;
            session.outstanding_ping = Some((nonce, now));
            session.pings_sent += 1;
        }
        self.send_link(peer, LinkPayload::Ping(Ping { nonce }));
    }

    fn free_slot(&self) -> Option<u8> {
        (1..self.rules.max_players.min(M::MAX_PLAYERS)).find(|slot| {
            !self
                .peers
                .values()
                .any(|session| session.phase == Phase::Ready && session.slot == *slot)
        })
    }

    fn drop_peer(&mut self, peer: PeerId, reason: LeaveReason, out: &mut Vec<SessionEvent<M>>) {
        self.forget_peer(peer, reason, out);
        self.transport.disconnect(peer);
    }

    fn forget_peer(&mut self, peer: PeerId, reason: LeaveReason, out: &mut Vec<SessionEvent<M>>) {
        if let Some(session) = self.peers.remove(&peer)
            && session.phase == Phase::Ready
        {
            if self.config.role == Role::Host {
                for other in self.ready_peers() {
                    self.send_link(other, LinkPayload::MemberLeft { slot: session.slot });
                }
            }
            out.push(SessionEvent::PlayerLeft {
                slot: session.slot,
                reason,
                peer,
            });
        }
    }

    fn tell_progress(&self, progress: LinkProgress) {
        if let Some(sink) = &self.progress {
            sink.send(progress);
        }
    }

    fn send_bare(&mut self, peer: PeerId, link: LinkPayload) {
        let message = M::from_link(link);
        let Ok(bytes) = encode(&message) else {
            return;
        };
        let _ = self.transport.send(peer, channel_for(&message), &bytes);
    }

    fn send_link(&mut self, peer: PeerId, link: LinkPayload) {
        self.send_to(peer, &M::from_link(link));
    }

    fn send_to(&mut self, peer: PeerId, message: &M) {
        let envelope = Envelope {
            from: self.local_slot.unwrap_or(0),
            to: None,
        };
        self.send_enveloped(peer, &envelope, message);
    }

    fn send_addressed(&mut self, slot: u8, message: &M) {
        let from = self.local_slot.unwrap_or(0);
        let envelope = Envelope {
            from,
            to: Some(slot),
        };
        let target = match self.config.role {
            Role::Host => self.ready_peer_in_slot(slot),
            Role::Guest => self.ready_peers().first().copied(),
        };
        if let Some(target) = target {
            self.send_enveloped(target, &envelope, message);
        }
    }

    fn send_enveloped(&mut self, peer: PeerId, envelope: &Envelope, message: &M) {
        let Ok(bytes) = encode_enveloped(envelope, message) else {
            return;
        };
        let _ = self.transport.send(peer, channel_for(message), &bytes);
    }
}

fn clamped_rules<M: GameMessage>(rules: SessionRules) -> SessionRules {
    SessionRules {
        mode: rules.mode,
        max_players: rules.max_players.clamp(2, M::MAX_PLAYERS.max(2)),
    }
}

fn channel_for<M: GameMessage>(message: &M) -> Channel {
    if message.is_latest_wins() {
        Channel::UnreliableLatest
    } else {
        Channel::ReliableOrdered
    }
}
