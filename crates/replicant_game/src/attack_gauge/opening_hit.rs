use replicant_proto::{EnemyHit, EnemyId};

use crate::attack_gauge::{
    BossGaugeState, HOST_REPLAYS_GUEST_HIT_AT_MOST_MS_BEFORE_ASK,
    HOST_WAITS_FOR_GUEST_OPENING_HIT_MS,
};

/// The last guest hit the host applied to a boss and when (ms on the host's clock). A guest's ask
/// is answered by replaying it as the host player's own hit, so the host's game opens its own
/// gauge: one opened by the mod's add-node and open calls lacks the boss's gauge state, so its
/// break is refused and it never closes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuestOpeningHits {
    last: Option<(EnemyHit, u64)>,
}

/// The hit to replay: the guest's flags, kind and hp damage (capped so the boss survives it; the
/// caller puts its hp back afterwards), no stagger damage, since the guest's hit already gave it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpeningHitReplay {
    pub hit: EnemyHit,
    pub age_ms: u64,
}

/// What replaying the hit did on the host: the boss's hp around it, whether the host's game
/// opened its own gauge from it, the boss's gauge state after it, and the boss's vtable RVA (the
/// script boss model's is 0xADF730).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpeningReplayReport {
    pub hp_before: i32,
    pub hp_after_hit: i32,
    pub hp_put_back: bool,
    pub gauge_opened: bool,
    pub boss_state: Option<BossGaugeState>,
    pub boss_vtable_rva: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpeningHitMiss {
    NoGuestHit,
    OtherBoss,
    TooOld,
    BossNearlyDead,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpeningHitStep {
    Replay(OpeningHitReplay),
    Wait,
    GiveUp(OpeningHitMiss),
}

impl GuestOpeningHits {
    pub const fn new() -> Self {
        Self { last: None }
    }

    pub fn remember(&mut self, hit: EnemyHit, at_ms: u64) {
        self.last = Some((hit, at_ms));
    }

    /// The step for an ask on `boss` that arrived `ms_since_ask` ago: replay the guest's last hit
    /// on that boss when it landed at most a moment before the ask (it is used once), wait a
    /// little when it has not arrived yet, else give up.
    pub fn step(
        &mut self,
        boss: EnemyId,
        now_ms: u64,
        ms_since_ask: u64,
        boss_hp: i32,
    ) -> OpeningHitStep {
        let miss = match self.last {
            None => OpeningHitMiss::NoGuestHit,
            Some((hit, _)) if hit.id != boss => OpeningHitMiss::OtherBoss,
            Some((hit, at_ms)) => {
                let age_ms = now_ms.saturating_sub(at_ms);
                if age_ms
                    > ms_since_ask.saturating_add(HOST_REPLAYS_GUEST_HIT_AT_MOST_MS_BEFORE_ASK)
                {
                    OpeningHitMiss::TooOld
                } else if boss_hp <= 1 {
                    OpeningHitMiss::BossNearlyDead
                } else {
                    self.last = None;
                    let most = u32::try_from(boss_hp - 1).unwrap_or(0);
                    return OpeningHitStep::Replay(OpeningHitReplay {
                        hit: EnemyHit {
                            hp_damage: hit.hp_damage.clamp(1, most),
                            second_gauge: 0,
                            third_gauge: 0,
                            ..hit
                        },
                        age_ms,
                    });
                }
            }
        };
        if miss != OpeningHitMiss::BossNearlyDead
            && ms_since_ask < HOST_WAITS_FOR_GUEST_OPENING_HIT_MS
        {
            return OpeningHitStep::Wait;
        }
        OpeningHitStep::GiveUp(miss)
    }
}
