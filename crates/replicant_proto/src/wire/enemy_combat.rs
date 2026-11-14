use serde::{Deserialize, Serialize};

use crate::wire::{
    EnemyId, MAX_HIT_DAMAGE, MAX_HIT_PUSH_STRENGTH, MAX_ITEM_COUNT, MAX_POSITION_ABS, MAX_WORD_ID,
    WireError,
};

/// The guest's hit on its copy of a host enemy, as the guest's game computed it (before the
/// enemy's defence). The host applies it to its own enemy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyHit {
    pub phase: u16,
    pub id: EnemyId,
    pub hit_id: u32,
    pub hp_damage: u32,
    pub second_gauge: u32,
    pub third_gauge: u32,
    pub flags: u32,
    pub attack_kind: u32,
}

/// A host enemy's hit that landed on the guest's body in the host's game. The guest applies it to
/// its own player, so its guard, dodge and downed rules decide what it does; the reaction bits of
/// `flags`, `push` and `direction` make its body react as the host's record says.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerHit {
    pub phase: u16,
    pub attacker: EnemyId,
    pub hp_damage: u32,
    pub flags: u32,
    pub attack_kind: u32,
    pub hit_id: u32,
    /// The record's push strength (its float at `+0x74`).
    pub push: f32,
    /// The record's hit direction in world space (`+0x90`), zero when the game left it unset.
    pub direction: [f32; 3],
}

/// A host enemy died. `by_guest` is true when the guest's reported hit was the killing blow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyKilled {
    pub phase: u16,
    pub id: EnemyId,
    pub by_guest: bool,
}

/// An item a host enemy dropped, for the guest to drop at its own copy of that enemy.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnemyDrop {
    pub phase: u16,
    pub id: EnemyId,
    pub item_id: u16,
    pub count: u8,
    pub position: [f32; 3],
}

/// A dead host enemy's body left the host's world, so the guest drops its copy's body too.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyBodyGone {
    pub phase: u16,
    pub id: EnemyId,
}

/// A word a host enemy dropped, for the guest to drop at its own copy of that enemy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyWordDrop {
    pub phase: u16,
    pub id: EnemyId,
    pub word_id: u16,
}

impl EnemyHit {
    pub fn validate(&self) -> Result<(), WireError> {
        self.id.validate()?;
        let damages = [self.hp_damage, self.second_gauge, self.third_gauge];
        if damages.iter().any(|damage| *damage > MAX_HIT_DAMAGE) {
            return Err(WireError::OutOfRange {
                field: "hit_damage",
            });
        }
        Ok(())
    }
}

impl PlayerHit {
    pub fn validate(&self) -> Result<(), WireError> {
        self.attacker.validate()?;
        if self.hp_damage > MAX_HIT_DAMAGE {
            return Err(WireError::OutOfRange {
                field: "hit_damage",
            });
        }
        if !(0.0..=MAX_HIT_PUSH_STRENGTH).contains(&self.push) {
            return Err(WireError::OutOfRange { field: "hit push" });
        }
        if self
            .direction
            .iter()
            .any(|axis| !axis.is_finite() || axis.abs() > MAX_POSITION_ABS)
        {
            return Err(WireError::OutOfRange {
                field: "hit direction",
            });
        }
        Ok(())
    }
}

impl EnemyDrop {
    pub fn validate(&self) -> Result<(), WireError> {
        self.id.validate()?;
        if self.count == 0 || self.count > MAX_ITEM_COUNT {
            return Err(WireError::OutOfRange { field: "count" });
        }
        if self
            .position
            .iter()
            .any(|axis| !axis.is_finite() || axis.abs() > MAX_POSITION_ABS)
        {
            return Err(WireError::OutOfRange {
                field: "drop_position",
            });
        }
        Ok(())
    }
}

impl EnemyWordDrop {
    pub fn validate(&self) -> Result<(), WireError> {
        self.id.validate()?;
        if self.word_id > MAX_WORD_ID {
            return Err(WireError::OutOfRange { field: "word_id" });
        }
        Ok(())
    }
}
