use crate::memory::GameMemory;
use crate::offsets::{
    ENTITY_RVA, FORCED_WEAPON_ID, MAX_WEAPON_LEVEL_STORED, ONE_HANDED_SWORD_KIND,
    SAVE_EQUIPPED_WEAPON_ID_OFFSET, SAVE_FORCED_WEAPON_FLAG_OFFSET, SAVE_WEAPON_LEVELS_OFFSET,
    WEAPON_RECORD_KIND_OFFSET, WEAPON_TABLE_LEN,
};

use super::KAINE_PLAYER_FLAG_SET;
use super::hit_power::{HeldWeapon, level_index, weapon_record};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceWeapon {
    pub id: i32,
    pub level: i8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceSource {
    Host,
    Fallback,
}

impl ReferenceSource {
    pub const fn described(self) -> &'static str {
        match self {
            Self::Host => "from the host",
            Self::Fallback => "fallback: the weakest one-handed sword of the table",
        }
    }
}

impl ReferenceWeapon {
    /// `None` while the forced weapon is in play: what the save reads then is not a weapon the
    /// player chose.
    pub fn read_equipped<M: GameMemory>(memory: &M) -> Option<Self> {
        let save = memory.module_base() + u64::from(ENTITY_RVA);
        if memory
            .read_i32(save + SAVE_FORCED_WEAPON_FLAG_OFFSET)
            .ok()?
            == KAINE_PLAYER_FLAG_SET
        {
            return None;
        }
        let id = memory
            .read_i32(save + SAVE_EQUIPPED_WEAPON_ID_OFFSET)
            .ok()
            .filter(|id| (0..WEAPON_TABLE_LEN).contains(id) && *id != FORCED_WEAPON_ID)?;
        let level = memory
            .read_u8(save + SAVE_WEAPON_LEVELS_OFFSET + u64::try_from(id).ok()?)
            .ok()?
            .cast_signed();
        Some(Self { id, level })
    }

    pub fn is_usable(self) -> bool {
        (0..WEAPON_TABLE_LEN).contains(&self.id)
            && self.id != FORCED_WEAPON_ID
            && (0..=MAX_WEAPON_LEVEL_STORED).contains(&self.level)
    }
}

pub(super) fn resolve_reference<M: GameMemory>(
    memory: &M,
    host_weapon: Option<ReferenceWeapon>,
) -> Option<(HeldWeapon, ReferenceSource)> {
    let from_host = host_weapon
        .filter(|weapon| weapon.is_usable())
        .and_then(|weapon| HeldWeapon::at_level(memory, weapon.id, weapon.level))
        .filter(deals_damage);
    if let Some(weapon) = from_host {
        return Some((weapon, ReferenceSource::Host));
    }
    let level = host_weapon.map_or(0, |weapon| weapon.level.clamp(0, MAX_WEAPON_LEVEL_STORED));
    weakest_one_handed_sword(memory, level).map(|weapon| (weapon, ReferenceSource::Fallback))
}

fn deals_damage(weapon: &HeldWeapon) -> bool {
    weapon
        .percents_at_level()
        .iter()
        .all(|percent| *percent > 0)
}

fn weakest_one_handed_sword<M: GameMemory>(memory: &M, level: i8) -> Option<HeldWeapon> {
    let index = level_index(level)?;
    (0..WEAPON_TABLE_LEN)
        .filter(|id| *id != FORCED_WEAPON_ID)
        .filter(|id| {
            weapon_record(memory, *id)
                .and_then(|record| memory.read_u8(record + WEAPON_RECORD_KIND_OFFSET).ok())
                == Some(ONE_HANDED_SWORD_KIND)
        })
        .filter_map(|id| HeldWeapon::at_level(memory, id, level))
        .filter(deals_damage)
        .min_by_key(|weapon| {
            (
                weapon.percents.hp[index],
                weapon.percents.second_gauge[index],
                weapon.percents.third_gauge[index],
                weapon.id,
            )
        })
}
