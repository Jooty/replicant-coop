use crate::blocks::distance_between;
use crate::bombs::{BOMB_HOLDER_HANDLE_OFFSET, BOMB_VTABLE_RVA, BreakableClass, breakable_class};
use crate::effects::read_actor_position;
use crate::enemies::ActorHandle;
use crate::game::GameError;
use crate::memory::GameMemory;
use crate::offsets::{
    ACTOR_TRANSFORM_BLOCK_BYTES, ACTOR_TRANSFORM_BLOCK_OFFSET, HANDLE_POOL_POINTERS_RVA,
    HANDLE_POOL_SLOT_COUNT,
};
use crate::puppet::PuppetPose;

const POINTER_BYTES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SightedBreakable {
    pub actor: u64,
    pub class: BreakableClass,
    pub position: [f32; 3],
}

/// A bomb actor with the actor that holds it, when someone does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SightedBomb {
    pub actor: u64,
    pub slot: u16,
    pub pose: PuppetPose,
    pub holder: Option<u64>,
}

/// Every actor in the game's handle pool with its slot, the pool's own order.
pub fn pool_actors<M: GameMemory>(memory: &M) -> Result<Vec<(u16, u64)>, GameError> {
    let mut table = vec![0_u8; HANDLE_POOL_SLOT_COUNT as usize * POINTER_BYTES];
    memory.read(
        memory.module_base() + u64::from(HANDLE_POOL_POINTERS_RVA),
        &mut table,
    )?;
    let (pointers, _) = table.as_chunks::<POINTER_BYTES>();
    Ok((0_u16..)
        .zip(pointers)
        .map(|(slot, bytes)| (slot, u64::from_le_bytes(*bytes)))
        .filter(|(_, actor)| *actor != 0)
        .collect())
}

pub fn read_breakables<M: GameMemory>(memory: &M) -> Result<Vec<SightedBreakable>, GameError> {
    Ok(pool_actors(memory)?
        .into_iter()
        .filter_map(|(_, actor)| {
            let class = breakable_class(memory, actor)?;
            let position = read_actor_position(memory, actor).ok()?;
            Some(SightedBreakable {
                actor,
                class,
                position,
            })
        })
        .collect())
}

pub fn nearest_breakable(
    breakables: &[SightedBreakable],
    position: [f32; 3],
    max_distance: f32,
) -> Option<SightedBreakable> {
    breakables
        .iter()
        .map(|breakable| (distance_between(breakable.position, position), breakable))
        .filter(|(distance, _)| *distance <= max_distance)
        .min_by(|(left, _), (right, _)| left.total_cmp(right))
        .map(|(_, breakable)| *breakable)
}

pub fn read_bombs<M: GameMemory>(memory: &M) -> Result<Vec<SightedBomb>, GameError> {
    let bomb_vtable = memory.module_base() + u64::from(BOMB_VTABLE_RVA);
    Ok(pool_actors(memory)?
        .into_iter()
        .filter(|(_, actor)| memory.read_u64(*actor) == Ok(bomb_vtable))
        .filter_map(|(slot, actor)| {
            let block = memory
                .read_array::<ACTOR_TRANSFORM_BLOCK_BYTES>(actor + ACTOR_TRANSFORM_BLOCK_OFFSET)
                .ok()?;
            let holder = ActorHandle::read(memory, actor + BOMB_HOLDER_HANDLE_OFFSET)
                .and_then(|handle| handle.resolve(memory));
            Some(SightedBomb {
                actor,
                slot,
                pose: PuppetPose::from_transform_block(&block),
                holder,
            })
        })
        .collect())
}
