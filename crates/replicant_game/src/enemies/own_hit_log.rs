use replicant_proto::PartnerBody;

use crate::kaine::level_fields_in_use;
use crate::memory::GameMemory;
use crate::offsets::{
    ACTOR_PLAYER_PARAM_POINTER_OFFSET, ENTITY_RVA, FORCED_WEAPON_ID,
    PLAYER_PARAM_CACHED_ATTACK_OFFSET, SAVE_EQUIPPED_WEAPON_ID_OFFSET,
    SAVE_FORCED_WEAPON_FLAG_OFFSET, SAVE_WEAPON_LEVELS_OFFSET, WEAPON_TABLE_LEN,
};

pub const OWN_HITS_LOGGED_EACH_PER_PHASE: u32 = 20;
pub const OWN_HITS_LOGGED_EVERY_NTH_AFTER: u32 = 10;
const ACTOR_DAMAGE_MODE_OFFSET: u64 = 0x6_B96C;
const ACTOR_DAMAGE_MODE_PERCENT_OFFSET: u64 = 0x6_B970;
const UNREADABLE: &str = "?";

#[derive(Debug, Clone, Default)]
pub struct OwnHitLimiter {
    phase: Option<u16>,
    seen: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct OwnHitInputs {
    pub weapon: Option<i32>,
    pub weapon_level: Option<u8>,
    pub level: Option<i32>,
    pub kaine_flag: Option<i32>,
    pub cached_attack: Option<(u32, f32)>,
    pub mode: Option<i32>,
    pub mode_percent: Option<i32>,
}

#[derive(Debug, Clone, Copy)]
pub struct OwnHitRecord {
    pub hp_damage: i32,
    pub second_gauge: i32,
    pub third_gauge: i32,
}

impl OwnHitLimiter {
    pub fn allow(&mut self, phase: u16) -> bool {
        if self.phase != Some(phase) {
            self.phase = Some(phase);
            self.seen = 0;
        }
        self.seen = self.seen.saturating_add(1);
        self.seen <= OWN_HITS_LOGGED_EACH_PER_PHASE
            || (self.seen - OWN_HITS_LOGGED_EACH_PER_PHASE)
                .is_multiple_of(OWN_HITS_LOGGED_EVERY_NTH_AFTER)
    }
}

impl OwnHitInputs {
    pub fn read<M: GameMemory>(memory: &M, player: u64) -> Self {
        let save = memory.module_base() + u64::from(ENTITY_RVA);
        let kaine_flag = memory.read_i32(save + SAVE_FORCED_WEAPON_FLAG_OFFSET).ok();
        let weapon = if kaine_flag == Some(1) {
            Some(FORCED_WEAPON_ID)
        } else {
            memory.read_i32(save + SAVE_EQUIPPED_WEAPON_ID_OFFSET).ok()
        };
        let weapon_level = weapon
            .and_then(|id| u8::try_from(id).ok())
            .filter(|id| i32::from(*id) < WEAPON_TABLE_LEN)
            .and_then(|id| {
                memory
                    .read_u8(save + SAVE_WEAPON_LEVELS_OFFSET + u64::from(id))
                    .ok()
            });
        let cached_attack = memory
            .read_u64(player + ACTOR_PLAYER_PARAM_POINTER_OFFSET)
            .ok()
            .filter(|param| *param != 0)
            .and_then(|param| {
                let address = param + PLAYER_PARAM_CACHED_ATTACK_OFFSET;
                Some((
                    memory.read_i32(address).ok()?.cast_unsigned(),
                    memory.read_f32(address).ok()?,
                ))
            });
        Self {
            weapon,
            weapon_level,
            level: memory
                .read_i32(
                    save + u64::from(level_fields_in_use(kaine_flag.unwrap_or_default()).level),
                )
                .ok(),
            kaine_flag,
            cached_attack,
            mode: memory.read_i32(player + ACTOR_DAMAGE_MODE_OFFSET).ok(),
            mode_percent: memory
                .read_i32(player + ACTOR_DAMAGE_MODE_PERCENT_OFFSET)
                .ok(),
        }
    }
}

pub fn spec_class_of(body: PartnerBody) -> &'static str {
    match body {
        PartnerBody::Nier => "PlayerBoy",
        PartnerBody::SeedKaineE => "PlayerKaine",
        PartnerBody::PrologueCharacter | PartnerBody::SeedNierY | PartnerBody::SeedNierF => {
            "PlayerDefault"
        }
    }
}

fn shown<T: ToString>(value: Option<T>) -> String {
    value.map_or_else(|| UNREADABLE.to_owned(), |value| value.to_string())
}

pub fn own_hit_text(
    inputs: &OwnHitInputs,
    record: OwnHitRecord,
    motion: Option<&str>,
    seed_and_class: Option<(&str, &str)>,
) -> String {
    let (seed, class) = seed_and_class.unwrap_or((UNREADABLE, UNREADABLE));
    let base = inputs.cached_attack.map_or_else(
        || UNREADABLE.to_owned(),
        |(raw, float)| format!("{raw:#x} ({float:.2})"),
    );
    format!(
        "own hit: weapon {} lv {}, level {}, kaine flag {}, base {base}, mode {}/{}, words n/a, record {}/{}/{}, motion {}, hp_damage {} (seed {seed}, class {class})",
        shown(inputs.weapon),
        shown(inputs.weapon_level),
        shown(inputs.level),
        shown(inputs.kaine_flag),
        shown(inputs.mode),
        shown(inputs.mode_percent),
        record.hp_damage,
        record.second_gauge,
        record.third_gauge,
        motion.unwrap_or(UNREADABLE),
        record.hp_damage,
    )
}
