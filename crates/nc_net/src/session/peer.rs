use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Handshaking,
    Ready,
    Refused { drop_at: Instant },
}

pub(super) struct PeerSession {
    pub(super) phase: Phase,
    pub(super) connected_at: Instant,
    pub(super) last_heard: Instant,
    pub(super) last_ping_sent: Instant,
    pub(super) outstanding_ping: Option<(u32, Instant)>,
    pub(super) rtt: Option<Duration>,
    pub(super) slot: u8,
    pub(super) name: String,
    pub(super) last_ticks: HashMap<u8, u32>,
    pub(super) pings_sent: u32,
    pub(super) pongs_received: u32,
}

impl PeerSession {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            phase: Phase::Handshaking,
            connected_at: now,
            last_heard: now,
            last_ping_sent: now,
            outstanding_ping: None,
            rtt: None,
            slot: 0,
            name: String::new(),
            last_ticks: HashMap::new(),
            pings_sent: 0,
            pongs_received: 0,
        }
    }

    pub(super) fn accepts_tick(&mut self, sender: u8, tick: u32) -> bool {
        let newer = match self.last_ticks.get(&sender) {
            None => true,
            Some(last) => (tick.wrapping_sub(*last) as i32) > 0,
        };
        if newer {
            self.last_ticks.insert(sender, tick);
        }
        newer
    }
}
