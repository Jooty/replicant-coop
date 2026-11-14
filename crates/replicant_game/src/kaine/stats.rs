use crate::memory::{GameMemory, MemoryError};
use crate::offsets::{
    ENTITY_HEALTH_OFFSET, ENTITY_MAGIC_OFFSET, ENTITY_RVA, PLAYER_PARAM_ARRAY_RVA,
    PLAYER_PARAM_MAX_HEALTH_OFFSET, PLAYER_PARAM_MAX_MAGIC_OFFSET,
};

use super::{
    KAINE_LEVEL_SAVE_OFFSET, KAINE_PLAYER_FLAG_SAVE_OFFSET, KAINE_PLAYER_FLAG_SET,
    LEVEL_STATS_ROW_BYTES, LEVEL_STATS_TABLE_RVA, MAX_LEVEL_STATS_ROW, STATS_ROW_TO_PARAM,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KaineStatsSynced {
    pub health: i32,
    pub max_health: i32,
    pub magic: f32,
    pub max_magic: f32,
}

impl KaineStatsSynced {
    pub fn log_line(&self) -> String {
        format!(
            "kaine: stats set to hp {}/{}, mp {}/{} (from her own level's row)",
            self.health, self.max_health, self.magic, self.max_magic
        )
    }
}

pub fn level_row_in_range(level: i32) -> bool {
    (0..=MAX_LEVEL_STATS_ROW).contains(&level)
}

pub fn param_patches(row: &[u8]) -> Vec<(u64, [u8; 4])> {
    if row.len() != LEVEL_STATS_ROW_BYTES {
        return Vec::new();
    }
    STATS_ROW_TO_PARAM
        .iter()
        .filter_map(|&(row_offset, param_offset)| {
            let bytes = row.get(row_offset..row_offset + 4)?.try_into().ok()?;
            Some((param_offset, bytes))
        })
        .collect()
}

pub fn clamped_current(health: i32, max_health: i32, magic: f32, max_magic: f32) -> (i32, f32) {
    (
        health.clamp(0, max_health.max(0)),
        magic.clamp(0.0, max_magic.max(0.0)),
    )
}

pub fn sync_kaine_stats<M: GameMemory>(
    memory: &mut M,
) -> Result<Option<KaineStatsSynced>, MemoryError> {
    let base = memory.module_base();
    let save = base + u64::from(ENTITY_RVA);
    if memory.read_i32(save + KAINE_PLAYER_FLAG_SAVE_OFFSET as u64)? != KAINE_PLAYER_FLAG_SET {
        return Ok(None);
    }
    let level = memory.read_i32(save + KAINE_LEVEL_SAVE_OFFSET as u64)?;
    let table = memory.read_u64(base + u64::from(LEVEL_STATS_TABLE_RVA))?;
    if !level_row_in_range(level) || table == 0 {
        return Ok(None);
    }
    let row = memory
        .read_array::<LEVEL_STATS_ROW_BYTES>(table + level as u64 * LEVEL_STATS_ROW_BYTES as u64)?;
    let param = base + u64::from(PLAYER_PARAM_ARRAY_RVA);
    let mut changed = false;
    for (offset, bytes) in param_patches(&row) {
        if memory.read_array::<4>(param + offset)? != bytes {
            memory.write(param + offset, &bytes)?;
            changed = true;
        }
    }
    if !changed {
        return Ok(None);
    }
    let max_health = memory.read_i32(param + u64::from(PLAYER_PARAM_MAX_HEALTH_OFFSET))?;
    let max_magic = memory.read_f32(param + u64::from(PLAYER_PARAM_MAX_MAGIC_OFFSET))?;
    let health_at = save + u64::from(ENTITY_HEALTH_OFFSET);
    let magic_at = save + u64::from(ENTITY_MAGIC_OFFSET);
    let (health, magic) = clamped_current(
        memory.read_i32(health_at)?,
        max_health,
        memory.read_f32(magic_at)?,
        max_magic,
    );
    memory.write_i32(health_at, health)?;
    memory.write_f32(magic_at, magic)?;
    Ok(Some(KaineStatsSynced {
        health,
        max_health,
        magic,
        max_magic,
    }))
}
