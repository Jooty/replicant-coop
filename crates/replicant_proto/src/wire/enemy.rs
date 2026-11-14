use serde::{Deserialize, Serialize};

use crate::wire::{
    COMPANION_ENEMY_GROUP, COMPANION_SLOT_COUNT, ENEMY_GROUP_COUNT, ENEMY_SPEC_BYTES,
    ENEMY_SPEC_SLOT_COUNT, MAX_ENEMIES_PER_SNAPSHOT, MAX_ENEMY_ACTOR_NAME_BYTES, MAX_ENEMY_SCALE,
    MAX_ENEMY_SNAPSHOT_PARTS, MAX_HP, MAX_POSITION_ABS, MAX_YAW_ABS_RADIANS, MotionState,
    WireError,
};

/// A network id for one enemy: its script group and the order it spawned in within that group
/// since the phase began. Never a handle or a spawn serial, which differ between machines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EnemyId {
    pub group: u16,
    pub ordinal: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnemyState {
    pub id: EnemyId,
    pub position: [f32; 3],
    pub yaw_radians: f32,
    pub motion: Option<MotionState>,
    pub hp: u32,
    pub max_hp: u32,
}

/// One part of the host's enemy snapshot for `tick`. A snapshot too big for one message is split
/// into `parts` messages that share the tick. `total` is how many enemies the host has in all and
/// `round` is how many of them this round holds; the two differ when the host has more than a round fits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnemySnapshot {
    pub tick: u32,
    pub phase: u16,
    pub part: u8,
    pub parts: u8,
    pub total: u16,
    pub round: u16,
    pub enemies: Vec<EnemyState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnemySpawned {
    pub phase: u16,
    pub enemy: EnemyState,
}

/// What the host's `PutEnemyActor` call used to create one enemy, so the guest can make the same
/// enemy when its own game never ran the script that spawned it. Sent right after the
/// `EnemySpawned` of the same id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnemyCreation {
    pub phase: u16,
    pub id: EnemyId,
    pub actor_name: String,
    pub spec_slot: u16,
    pub spawn_flag: u8,
    pub attach_flag: u8,
    pub scale: f32,
    pub spec: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnemyGoneReason {
    Killed,
    Removed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyGone {
    pub phase: u16,
    pub id: EnemyId,
    pub reason: EnemyGoneReason,
}

impl EnemyId {
    /// The fixed id of one of the game's own companions: they belong to no script group, so
    /// the id is the same on both machines without a roster.
    pub fn companion(slot: u16) -> Self {
        Self {
            group: COMPANION_ENEMY_GROUP,
            ordinal: slot,
        }
    }

    pub fn is_companion(&self) -> bool {
        self.group == COMPANION_ENEMY_GROUP
    }

    pub fn validate(&self) -> Result<(), WireError> {
        if self.is_companion() {
            if self.ordinal >= COMPANION_SLOT_COUNT {
                return Err(WireError::OutOfRange {
                    field: "companion_slot",
                });
            }
            return Ok(());
        }
        if self.group >= ENEMY_GROUP_COUNT {
            return Err(WireError::OutOfRange {
                field: "enemy_group",
            });
        }
        Ok(())
    }
}

impl EnemyCreation {
    pub fn validate(&self) -> Result<(), WireError> {
        self.id.validate()?;
        let name_ok = !self.actor_name.is_empty()
            && self.actor_name.len() <= MAX_ENEMY_ACTOR_NAME_BYTES
            && self
                .actor_name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
        if !name_ok {
            return Err(WireError::OutOfRange {
                field: "enemy_actor_name",
            });
        }
        if !self.scale.is_finite() || self.scale <= 0.0 || self.scale > MAX_ENEMY_SCALE {
            return Err(WireError::OutOfRange {
                field: "enemy_scale",
            });
        }
        if self.spec_slot >= ENEMY_SPEC_SLOT_COUNT {
            return Err(WireError::OutOfRange {
                field: "enemy_spec_slot",
            });
        }
        if self.spec.len() != ENEMY_SPEC_BYTES {
            return Err(WireError::OutOfRange {
                field: "enemy_spec",
            });
        }
        Ok(())
    }
}

impl EnemyState {
    pub fn validate(&self) -> Result<(), WireError> {
        self.id.validate()?;
        if self
            .position
            .iter()
            .any(|axis| !axis.is_finite() || axis.abs() > MAX_POSITION_ABS)
        {
            return Err(WireError::OutOfRange {
                field: "enemy_position",
            });
        }
        if !self.yaw_radians.is_finite() || self.yaw_radians.abs() > MAX_YAW_ABS_RADIANS {
            return Err(WireError::OutOfRange { field: "enemy_yaw" });
        }
        if let Some(motion) = &self.motion {
            motion.validate()?;
        }
        if self.max_hp > MAX_HP || self.hp > self.max_hp {
            return Err(WireError::OutOfRange { field: "enemy_hp" });
        }
        Ok(())
    }
}

impl EnemySnapshot {
    pub fn validate(&self) -> Result<(), WireError> {
        if self.parts == 0 || self.parts > MAX_ENEMY_SNAPSHOT_PARTS || self.part >= self.parts {
            return Err(WireError::OutOfRange {
                field: "snapshot_part",
            });
        }
        if self.round > self.total
            || usize::from(self.round)
                > MAX_ENEMIES_PER_SNAPSHOT * usize::from(MAX_ENEMY_SNAPSHOT_PARTS)
        {
            return Err(WireError::OutOfRange {
                field: "snapshot_round",
            });
        }
        if self.enemies.len() > MAX_ENEMIES_PER_SNAPSHOT {
            return Err(WireError::OutOfRange {
                field: "snapshot_enemies",
            });
        }
        self.enemies.iter().try_for_each(EnemyState::validate)
    }
}
