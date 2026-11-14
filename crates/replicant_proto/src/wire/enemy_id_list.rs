use serde::{Deserialize, Serialize};

use crate::wire::{EnemyId, MAX_ENEMY_ID_LIST_PARTS, MAX_ENEMY_IDS_PER_LIST_PART, WireError};

/// One part of the ids of every enemy the host has, sent at a low rate so the guest can tell an
/// enemy the host has but left out of a snapshot round from one the host does not have.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnemyIdList {
    pub phase: u16,
    pub tick: u32,
    pub part: u8,
    pub parts: u8,
    pub ids: Vec<EnemyId>,
}

impl EnemyIdList {
    pub fn validate(&self) -> Result<(), WireError> {
        if self.parts == 0 || self.parts > MAX_ENEMY_ID_LIST_PARTS || self.part >= self.parts {
            return Err(WireError::OutOfRange {
                field: "id_list_part",
            });
        }
        if self.ids.len() > MAX_ENEMY_IDS_PER_LIST_PART {
            return Err(WireError::OutOfRange {
                field: "id_list_ids",
            });
        }
        self.ids.iter().try_for_each(EnemyId::validate)
    }
}
