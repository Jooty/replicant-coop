use crate::kaine::{SpellBrightnessGroup, SpellLookPair, SpellSizeGroup};

pub const SPELL_EFFECT_TALLY_INTERVAL_MS: u64 = 1_000;
pub const HER_EFFECT_ROLES: [HerEffectRole; 4] = [
    HerEffectRole::Spawn,
    HerEffectRole::Shot,
    HerEffectRole::Hit,
    HerEffectRole::Circle,
];

/// What one of her effects is in a cast: the muzzle effect, the shot, its hit, or a circle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HerEffectRole {
    Spawn,
    Shot,
    Hit,
    Circle,
}

/// Her effects made on this side while casting, and the shot slot spawns kept from being sent
/// a second time, counted over one window and logged once per window.
#[derive(Debug, Default)]
pub struct SpellEffectTally {
    made: [u32; HER_EFFECT_ROLES.len()],
    shot_slots_not_resent: u32,
    window_start_ms: Option<u64>,
}

impl HerEffectRole {
    pub fn label(self) -> &'static str {
        match self {
            Self::Spawn => "spawn",
            Self::Shot => "shot",
            Self::Hit => "hit",
            Self::Circle => "circle",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Spawn => 0,
            Self::Shot => 1,
            Self::Hit => 2,
            Self::Circle => 3,
        }
    }
}

impl SpellEffectTally {
    pub fn count_made(&mut self, role: HerEffectRole, now_ms: u64) {
        self.window_start_ms.get_or_insert(now_ms);
        self.made[role.index()] += 1;
    }

    pub fn count_shot_slot_not_resent(&mut self, now_ms: u64) {
        self.window_start_ms.get_or_insert(now_ms);
        self.shot_slots_not_resent += 1;
    }

    pub fn made(&self, role: HerEffectRole) -> u32 {
        self.made[role.index()]
    }

    /// The window's line once it has run its length, and a fresh window; None before that or
    /// when nothing was counted.
    pub fn take_line(&mut self, now_ms: u64) -> Option<String> {
        let start = self.window_start_ms?;
        if now_ms.saturating_sub(start) < SPELL_EFFECT_TALLY_INTERVAL_MS {
            return None;
        }
        let line = self.line();
        *self = Self::default();
        Some(line)
    }

    fn line(&self) -> String {
        let made = HER_EFFECT_ROLES
            .map(|role| format!("{} {}", self.made(role), role.label()))
            .join(", ");
        let shots = self.made(HerEffectRole::Shot);
        let per_shot = if shots == 0 {
            "no shot".to_owned()
        } else {
            let ratio = |role| f64::from(self.made(role)) / f64::from(shots);
            format!(
                "per shot {:.2} spawn, {:.2} hit",
                ratio(HerEffectRole::Spawn),
                ratio(HerEffectRole::Hit)
            )
        };
        format!(
            "spell look: last 1 s her effects made {made} ({per_shot}); {} shot slot effects not sent again as world effects (the shot sync draws them)",
            self.shot_slots_not_resent
        )
    }
}

pub fn her_effect_role(pair: &SpellLookPair) -> HerEffectRole {
    match (pair.brightness_group, pair.size_group) {
        (Some(SpellBrightnessGroup::Spawns), _) => HerEffectRole::Spawn,
        (Some(SpellBrightnessGroup::Hits), _) | (None, SpellSizeGroup::Hits) => HerEffectRole::Hit,
        (None, SpellSizeGroup::Shots) => HerEffectRole::Shot,
        (None, SpellSizeGroup::Circles) => HerEffectRole::Circle,
    }
}
