use replicant_proto::ENEMY_GROUP_COUNT;

use crate::enemies::EnemySighting;
use crate::game::GameError;
use crate::memory::GameMemory;
use crate::npcs::read_enemy_body_layer;
use crate::offsets::{
    ACTOR_CLASS_ID_OFFSET, ACTOR_KIND_MASK_OFFSET, ACTOR_MANAGER_GROUP_COUNT_OFFSET,
    ACTOR_MANAGER_GROUP_STRIDE, ACTOR_MANAGER_RVA, ACTOR_SPAWN_SERIAL_OFFSET,
    ACTOR_TRANSFORM_BLOCK_BYTES, ACTOR_TRANSFORM_BLOCK_OFFSET, ENEMY_KIND_MASK_BITS,
    MAX_ACTORS_SCANNED_PER_GROUP, PUPPET_ACTOR_GROUP,
};
use crate::puppet::{PuppetPose, actor_is_live};

/// Lists every live enemy-like actor in the actor manager's script groups, in group order.
/// `actor_in_group(group, index)` is the game's `GetActorInGroup` and `hp_of(actor)` reads
/// `(hp, max_hp)` through the actor's parameter object; both are calls the caller makes.
pub fn scan_enemies<M: GameMemory>(
    memory: &M,
    skipped_actor: Option<u64>,
    partner_actors: &[u64],
    mut actor_in_group: impl FnMut(u16, i32) -> u64,
    mut hp_of: impl FnMut(u64) -> Option<(i32, i32)>,
) -> Result<Vec<EnemySighting>, GameError> {
    let counts = read_group_counts(memory)?;
    let mut sightings = Vec::new();
    for (group, count) in (0..ENEMY_GROUP_COUNT).zip(counts) {
        if i32::from(group) == PUPPET_ACTOR_GROUP {
            continue;
        }
        for index in 0..count.clamp(0, MAX_ACTORS_SCANNED_PER_GROUP) {
            let actor = actor_in_group(group, index);
            if actor == 0 || Some(actor) == skipped_actor || partner_actors.contains(&actor) {
                continue;
            }
            if let Some(sighting) = sight(memory, group, actor, &mut hp_of) {
                sightings.push(sighting);
            }
        }
    }
    Ok(sightings)
}

pub(crate) fn read_group_counts<M: GameMemory>(memory: &M) -> Result<Vec<i32>, GameError> {
    let stride = ACTOR_MANAGER_GROUP_STRIDE as usize;
    let mut table = vec![0_u8; usize::from(ENEMY_GROUP_COUNT) * stride];
    let first =
        memory.module_base() + u64::from(ACTOR_MANAGER_RVA) + ACTOR_MANAGER_GROUP_COUNT_OFFSET;
    memory.read(first, &mut table)?;
    Ok(table
        .chunks_exact(stride)
        .map(|record| i32::from_le_bytes([record[0], record[1], record[2], record[3]]))
        .collect())
}

/// How many actors the actor manager holds in script group `group` (the group number is the
/// script id the scripts address an enemy by); `None` when unreadable or out of range.
pub fn read_group_count<M: GameMemory>(memory: &M, group: u16) -> Option<i32> {
    if group >= ENEMY_GROUP_COUNT {
        return None;
    }
    let at = memory.module_base()
        + u64::from(ACTOR_MANAGER_RVA)
        + ACTOR_MANAGER_GROUP_COUNT_OFFSET
        + u64::from(group) * ACTOR_MANAGER_GROUP_STRIDE;
    memory.read_i32(at).ok()
}

fn sight<M: GameMemory>(
    memory: &M,
    group: u16,
    actor: u64,
    hp_of: &mut impl FnMut(u64) -> Option<(i32, i32)>,
) -> Option<EnemySighting> {
    let kind_mask = memory.read_i32(actor + ACTOR_KIND_MASK_OFFSET).ok()?;
    if kind_mask.cast_unsigned() & ENEMY_KIND_MASK_BITS == 0 {
        return None;
    }
    if !actor_is_live(memory, actor).ok()? {
        return None;
    }
    let spawn_serial = memory.read_i32(actor + ACTOR_SPAWN_SERIAL_OFFSET).ok()?;
    let block = memory
        .read_array::<ACTOR_TRANSFORM_BLOCK_BYTES>(actor + ACTOR_TRANSFORM_BLOCK_OFFSET)
        .ok()?;
    let pose = PuppetPose::from_transform_block(&block);
    let (hp, max_hp) = hp_of(actor)?;
    Some(EnemySighting {
        actor,
        spawn_serial,
        group,
        class_id: memory.read_i32(actor + ACTOR_CLASS_ID_OFFSET).unwrap_or(-1),
        position: [pose.position.x, pose.position.y, pose.position.z],
        yaw_radians: pose.yaw,
        motion: read_enemy_body_layer(memory, actor)
            .ok()
            .flatten()
            .map(|layer| layer.motion),
        hp,
        max_hp,
    })
}
