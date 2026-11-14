use replicant_proto::AttackGaugeAsk;

use crate::attack_gauge::HOST_REFUSES_GAUGE_ASK_AFTER_CLOSE_MS;

/// What the host knows when the guest asks it to open a gauge: its phase, whether its own gauge
/// is open, whether the ask's boss (and second actor, when the ask has one) are here, and how long
/// ago its last gauge closed (`None` when none has closed in this phase).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GaugeAskFacts {
    pub local_phase: u16,
    pub switched_off: bool,
    pub host_gauge_open: bool,
    pub boss_found: bool,
    pub partner_found: bool,
    pub ms_since_host_close: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GaugeAskRefusal {
    SwitchedOff,
    OtherPhase,
    Repeated,
    HostGaugeOpen,
    JustClosed,
    BossMissing,
    PartnerMissing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GaugeAskVerdict {
    ReplayOpeningHit,
    Refuse(GaugeAskRefusal),
}

/// The host's side of the guest's asks: each ask is answered once, by its number.
#[derive(Clone, Debug, Default)]
pub struct HostGaugeAsks {
    last_ask: Option<u32>,
}

impl HostGaugeAsks {
    pub const fn new() -> Self {
        Self { last_ask: None }
    }

    /// Lets the host's game open its own gauge (by replaying the guest's opening hit as the host
    /// player's) unless the host already has one, has just closed one (the guest's copy may be
    /// reopening the gauge the host just broke) or lacks the actors.
    pub fn judge(&mut self, facts: &GaugeAskFacts, ask: &AttackGaugeAsk) -> GaugeAskVerdict {
        if facts.switched_off {
            return GaugeAskVerdict::Refuse(GaugeAskRefusal::SwitchedOff);
        }
        if ask.phase != facts.local_phase {
            return GaugeAskVerdict::Refuse(GaugeAskRefusal::OtherPhase);
        }
        if self.last_ask == Some(ask.ask) {
            return GaugeAskVerdict::Refuse(GaugeAskRefusal::Repeated);
        }
        self.last_ask = Some(ask.ask);
        if facts.host_gauge_open {
            return GaugeAskVerdict::Refuse(GaugeAskRefusal::HostGaugeOpen);
        }
        if facts
            .ms_since_host_close
            .is_some_and(|since| since < HOST_REFUSES_GAUGE_ASK_AFTER_CLOSE_MS)
        {
            return GaugeAskVerdict::Refuse(GaugeAskRefusal::JustClosed);
        }
        if !facts.boss_found {
            return GaugeAskVerdict::Refuse(GaugeAskRefusal::BossMissing);
        }
        if ask.open.partner.is_some() && !facts.partner_found {
            return GaugeAskVerdict::Refuse(GaugeAskRefusal::PartnerMissing);
        }
        GaugeAskVerdict::ReplayOpeningHit
    }
}
