use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::Instant;

use replicant_game::enemies::AttackRecord;
use replicant_game::kaine::{
    ReferenceWeapon, SwappedBodyPower, SwappedPowerLog, read_minimum_damages, swapped_power_line,
    weapon_powered_source,
};
use replicant_proto::PartnerBody;

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::memory::InProcessMemory;
use crate::player_body::real_body;

static POWER_LOG: Mutex<SwappedPowerLog> = Mutex::new(SwappedPowerLog::new());
static HOST_WEAPON: Mutex<Option<ReferenceWeapon>> = Mutex::new(None);
static POWER_LOG_CLOCK: LazyLock<Instant> = LazyLock::new(Instant::now);

pub(crate) fn host_weapon_reported(weapon: Option<ReferenceWeapon>) {
    *HOST_WEAPON.lock().unwrap_or_else(PoisonError::into_inner) = weapon;
}

pub(crate) fn hit_at_first_body_power(
    build: &VerifiedBuild,
    player: u64,
    record: u64,
    attack: AttackRecord,
) -> AttackRecord {
    let swapped =
        real_body().is_some_and(|real| real.by_swap && real.body == PartnerBody::SeedKaineE);
    if !swapped || !weapon_powered_source(attack.source_class) {
        return attack;
    }
    let memory = InProcessMemory::new(build);
    let host_weapon = *HOST_WEAPON.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(power) = SwappedBodyPower::read(&memory, player, host_weapon) else {
        return attack;
    };
    let [hp_damage, second_gauge, third_gauge] = power.scaled(
        [attack.hp_damage, attack.second_gauge, attack.third_gauge],
        read_minimum_damages(&memory, record),
    );
    let now_ms = u64::try_from(POWER_LOG_CLOCK.elapsed().as_millis()).unwrap_or(u64::MAX);
    let tally = POWER_LOG
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .note_hit(now_ms, attack.hp_damage, hp_damage);
    if let Some(tally) = tally {
        log_line(&swapped_power_line(&power, tally));
    }
    AttackRecord {
        hp_damage,
        second_gauge,
        third_gauge,
        ..attack
    }
}
