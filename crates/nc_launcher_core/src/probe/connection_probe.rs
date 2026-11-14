use std::thread;
use std::time::{Duration, Instant};

use nc_net::{Invite, Role, Session, SessionConfig, SessionEvent, Transport};
use replicant_proto::{Message, RejectReason};

use crate::report::{ConnectionReport, PathKind, RttStats, judge};
use crate::{
    INVALID_TICKET_TEXT, LauncherConfig, NetError, PROBE_MAX_SECONDS, PROBE_MIN_SECONDS,
    PROBE_SAMPLE_INTERVAL, STORY_RULES, TransportFactory, WORKER_WAKE_INTERVAL,
};

const HOST_SLOT: u8 = 0;

#[derive(Default)]
struct Gathered {
    peer: Option<nc_net::PeerId>,
    host_name: Option<String>,
    rejected: Option<RejectReason>,
    rtts: Vec<Duration>,
    direct: Option<bool>,
    pings_sent: u32,
    pings_lost: u32,
}

impl Gathered {
    fn absorb(&mut self, events: &[SessionEvent<Message>]) {
        for event in events {
            match event {
                SessionEvent::PlayerJoined { slot, name, peer } => {
                    self.peer = Some(*peer);
                    if *slot == HOST_SLOT {
                        self.host_name = Some(name.clone());
                    }
                }
                SessionEvent::Rejected { reason } => self.rejected = Some(*reason),
                SessionEvent::PlayerLeft { .. } => self.peer = None,
                _ => {}
            }
        }
    }

    fn sample(&mut self, session: &Session<Box<dyn Transport>, Message>, peer: nc_net::PeerId) {
        let stats = session.transport().link_stats(peer);
        let rtt = stats
            .and_then(|stats| stats.rtt)
            .or_else(|| session.rtt(peer));
        if let Some(rtt) = rtt {
            self.rtts.push(rtt);
        }
        if let Some(stats) = stats {
            self.direct = Some(stats.direct);
        }
    }

    fn finish_counts(
        &mut self,
        session: &Session<Box<dyn Transport>, Message>,
        peer: nc_net::PeerId,
    ) {
        let Some(counts) = session.ping_counts(peer) else {
            return;
        };
        let settled = counts.sent - u32::from(counts.awaiting_answer);
        self.pings_sent = settled;
        self.pings_lost = settled.saturating_sub(counts.answered);
    }

    fn into_report(self) -> ConnectionReport {
        let reachable = !self.rtts.is_empty() || self.rejected.is_some();
        let path = match self.direct {
            Some(true) => PathKind::Direct,
            Some(false) => PathKind::Relay,
            None => PathKind::Unknown,
        };
        let rtt = RttStats::from_samples(&self.rtts);
        let loss_percent = if self.pings_sent == 0 {
            0.0
        } else {
            self.pings_lost as f32 * 100.0 / self.pings_sent as f32
        };
        ConnectionReport {
            reachable,
            path,
            rtt,
            loss_percent,
            rejected: self.rejected,
            host_name: self.host_name,
            verdict: judge(reachable, self.rejected, path, rtt, loss_percent).summary,
        }
    }
}

pub(crate) fn run_probe(
    factory: &TransportFactory,
    config: &LauncherConfig,
    ticket: &str,
    seconds: u32,
) -> Result<ConnectionReport, NetError> {
    let invite = Invite::new(ticket.trim().to_owned())
        .map_err(|_| NetError::Failed(INVALID_TICKET_TEXT.to_owned()))?;
    let transport = factory().map_err(|error| NetError::Failed(error.to_string()))?;
    let mut session = Session::new(
        transport,
        SessionConfig {
            role: Role::Guest,
            name: config.name.clone(),
            build: config.build,
            rules: STORY_RULES,
        },
    );
    session
        .join(&invite)
        .map_err(|error| NetError::Failed(error.to_string()))?;
    let seconds = seconds.clamp(PROBE_MIN_SECONDS, PROBE_MAX_SECONDS);
    let deadline = Instant::now() + Duration::from_secs(u64::from(seconds));
    let mut gathered = Gathered::default();
    let mut last_sample: Option<Instant> = None;
    while Instant::now() < deadline && gathered.rejected.is_none() {
        let now = Instant::now();
        let mut events = Vec::new();
        session.poll(now, &mut events);
        gathered.absorb(&events);
        if let Some(peer) = gathered.peer
            && last_sample.is_none_or(|at| now.duration_since(at) >= PROBE_SAMPLE_INTERVAL)
        {
            last_sample = Some(now);
            gathered.sample(&session, peer);
        }
        thread::sleep(WORKER_WAKE_INTERVAL);
    }
    if let Some(peer) = gathered.peer {
        gathered.finish_counts(&session, peer);
    }
    session.close();
    Ok(gathered.into_report())
}
