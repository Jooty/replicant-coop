use replicant_proto::EnemyId;

use crate::attack_gauge::{
    GUEST_ASKED_GAUGE_GRACE_MS, GUEST_GAUGE_REMAINING_FLOOR_SECONDS, GUEST_ONLY_GAUGE_GRACE_MS,
};

/// The host's open gauge as the guest last heard it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostGaugeView {
    pub phase: u16,
    pub serial: u32,
    pub boss: EnemyId,
    pub partner: Option<EnemyId>,
}

/// The guest's own gauge: the shared enemy its boss handle names (`None` when the boss is not a
/// shared enemy), and the opening of the host's it was opened or taken over for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalGaugeView {
    pub boss: Option<EnemyId>,
    pub mirrored_serial: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuestGaugeFacts {
    pub local_phase: u16,
    pub host: Option<HostGaugeView>,
    pub local: Option<LocalGaugeView>,
    pub boss_copy_found: bool,
    pub partner_copy_found: bool,
    pub local_only_for_ms: u64,
    pub asked_host: bool,
    pub switched_off: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuestGaugeWait {
    BossCopyMissing,
    PartnerCopyMissing,
    GuestOnlyGrace,
    HostAsked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuestGaugeStep {
    Nothing,
    Open,
    AskHost,
    Mirror,
    CloseHostClosed,
    CloseOtherOpening,
    CloseHostHasNone,
    Wait(GuestGaugeWait),
    LeaveToGame,
}

/// What the guest does with its own gauge this tick. The host's gauge decides: the guest opens one
/// like it on its copy of the boss, follows the same opening, and closes its own when the host's
/// closed or is another opening. A gauge the guest's own game opened on a shared boss is asked of
/// the host once; it closes when the host's never came (after a grace).
pub fn guest_gauge_step(facts: &GuestGaugeFacts) -> GuestGaugeStep {
    if facts.switched_off {
        return GuestGaugeStep::LeaveToGame;
    }
    let host = facts.host.filter(|host| host.phase == facts.local_phase);
    match (host, facts.local) {
        (None, None) => GuestGaugeStep::Nothing,
        (None, Some(local)) if local.mirrored_serial.is_some() => GuestGaugeStep::CloseHostClosed,
        (None, Some(local)) if local.boss.is_some() && !facts.asked_host => GuestGaugeStep::AskHost,
        (None, Some(_))
            if facts.asked_host && facts.local_only_for_ms < GUEST_ASKED_GAUGE_GRACE_MS =>
        {
            GuestGaugeStep::Wait(GuestGaugeWait::HostAsked)
        }
        (None, Some(_))
            if !facts.asked_host && facts.local_only_for_ms < GUEST_ONLY_GAUGE_GRACE_MS =>
        {
            GuestGaugeStep::Wait(GuestGaugeWait::GuestOnlyGrace)
        }
        (None, Some(_)) => GuestGaugeStep::CloseHostHasNone,
        (Some(host), Some(local)) => {
            let other_serial = local
                .mirrored_serial
                .is_some_and(|serial| serial != host.serial);
            if other_serial || local.boss != Some(host.boss) {
                GuestGaugeStep::CloseOtherOpening
            } else {
                GuestGaugeStep::Mirror
            }
        }
        (Some(host), None) => {
            if !facts.boss_copy_found {
                GuestGaugeStep::Wait(GuestGaugeWait::BossCopyMissing)
            } else if host.partner.is_some() && !facts.partner_copy_found {
                GuestGaugeStep::Wait(GuestGaugeWait::PartnerCopyMissing)
            } else {
                GuestGaugeStep::Open
            }
        }
    }
}

/// The hp the guest shows for a node the host reports at `host_hp`: the host's value, and 0 for a
/// node the host broke.
pub fn guest_node_hp(host_hp: i32) -> i32 {
    host_hp.max(0)
}

/// The hp the guest shows right after its own hit took `damage`, before the host answers: never
/// below 1, so the guest's game never breaks a node itself.
pub fn guest_node_hp_after_own_hit(shown_hp: i32, damage: i32) -> i32 {
    if shown_hp < 1 {
        return shown_hp;
    }
    shown_hp.saturating_sub(damage).max(1)
}

/// The guest's gauge clock never runs out on its own: the host's closing ends it.
pub fn guest_remaining_floor(remaining_seconds: f32) -> Option<f32> {
    (remaining_seconds < GUEST_GAUGE_REMAINING_FLOOR_SECONDS)
        .then_some(GUEST_GAUGE_REMAINING_FLOOR_SECONDS)
}
