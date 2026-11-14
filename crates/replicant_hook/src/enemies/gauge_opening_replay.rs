use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::Instant;

use replicant_game::Replicant;
use replicant_game::attack_gauge::{
    GuestOpeningHits, OpeningHitMiss, OpeningHitReplay, OpeningHitStep,
};
use replicant_game::enemies::unpack_hit_kind;
use replicant_proto::{EnemyHit, EnemyId};

use crate::build_gate::VerifiedBuild;
use crate::enemies::combat_view::as_remote_with_kind;
use crate::enemies::game_calls::EnemyCalls;
use crate::memory::InProcessMemory;

static OPENING_HITS: Mutex<GuestOpeningHits> = Mutex::new(GuestOpeningHits::new());
static OPENING_HIT_CLOCK: LazyLock<Instant> = LazyLock::new(Instant::now);

/// The guest's opening hit, applied on the host's boss with the host's player as the attacker.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ReplayedOpeningHit {
    pub(crate) replay: OpeningHitReplay,
    pub(crate) hp_before: i32,
    pub(crate) hp_after_hit: i32,
    pub(crate) hp_put_back: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum OpeningReplayStep {
    Wait,
    GiveUp(OpeningHitMiss),
    CannotReplay(&'static str),
    Replayed(ReplayedOpeningHit),
}

pub(crate) fn remember_guest_boss_hit(hit: &EnemyHit) {
    OPENING_HITS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remember(*hit, clock_ms());
}

/// Answers a guest's ask on the host's boss `boss_actor` (id `boss`) that arrived `ms_since_ask`
/// ago: the guest's hit that opened its own gauge is applied again as the host player's own hit,
/// with the guest's flags and kind, so the boss's script sees what it sees for a host hit and
/// opens the host's own gauge; the boss's hp is then put back, since the guest's hit already
/// took it.
pub(crate) fn replay_guest_opening_hit(
    build: &VerifiedBuild,
    boss_actor: u64,
    boss: EnemyId,
    ms_since_ask: u64,
) -> OpeningReplayStep {
    let memory = InProcessMemory::new(build);
    let calls = EnemyCalls::new(build);
    let Some((hp_before, _)) = calls.hp_of(&memory, boss_actor) else {
        return OpeningReplayStep::CannotReplay("the boss's hp is not readable");
    };
    let Ok(player) = Replicant::new(InProcessMemory::new(build)).playable_actor() else {
        return OpeningReplayStep::CannotReplay("your player is not readable");
    };
    let step = OPENING_HITS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .step(boss, clock_ms(), ms_since_ask, hp_before);
    let replay = match step {
        OpeningHitStep::Wait => return OpeningReplayStep::Wait,
        OpeningHitStep::GiveUp(miss) => return OpeningReplayStep::GiveUp(miss),
        OpeningHitStep::Replay(replay) => replay,
    };
    let hit = replay.hit;
    let (kind, source_class) = unpack_hit_kind(hit.attack_kind);
    as_remote_with_kind(kind, source_class, || {
        calls.direct_hit(
            player,
            boss_actor,
            [hit.hp_damage, hit.second_gauge, hit.third_gauge],
            hit.flags,
        );
    });
    let hp_after_hit = calls
        .hp_of(&memory, boss_actor)
        .map_or(hp_before, |(hp, _)| hp);
    let hp_put_back = hp_after_hit == hp_before || calls.set_hp(&memory, boss_actor, hp_before);
    OpeningReplayStep::Replayed(ReplayedOpeningHit {
        replay,
        hp_before,
        hp_after_hit,
        hp_put_back,
    })
}

fn clock_ms() -> u64 {
    u64::try_from(OPENING_HIT_CLOCK.elapsed().as_millis()).unwrap_or(u64::MAX)
}
