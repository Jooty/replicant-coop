use crate::memory::GameMemory;
use crate::npcs::motion_layers_text;
use crate::offsets::{
    ACTOR_CLASS_ID_OFFSET, ACTOR_DRAWN_FLAG_OFFSET, ACTOR_TRANSFORM_BLOCK_BYTES,
    ACTOR_TRANSFORM_BLOCK_OFFSET, PARTNER_KAINE_ACTOR_RVA, PARTNER_THIRD_MEMBER_ACTOR_RVA,
    PARTNER_VARIANT_POINTER_RVA,
};
use crate::puppet::{PuppetPose, actor_is_live};

pub fn partner_actors<M: GameMemory>(memory: &M) -> Vec<u64> {
    let base = memory.module_base();
    let variant = memory
        .read_u64(base + u64::from(PARTNER_VARIANT_POINTER_RVA))
        .unwrap_or(0);
    [
        base + u64::from(PARTNER_KAINE_ACTOR_RVA),
        base + u64::from(PARTNER_THIRD_MEMBER_ACTOR_RVA),
        variant,
    ]
    .into_iter()
    .filter(|actor| *actor != 0)
    .collect()
}

/// Unique actor -2's place and facing, when the game holds her live.
pub fn unique_companion_pose<M: GameMemory>(memory: &M) -> Option<PuppetPose> {
    let actor = memory.module_base() + u64::from(PARTNER_KAINE_ACTOR_RVA);
    if !actor_is_live(memory, actor).unwrap_or(false) {
        return None;
    }
    let block = memory
        .read_array::<ACTOR_TRANSFORM_BLOCK_BYTES>(actor + ACTOR_TRANSFORM_BLOCK_OFFSET)
        .ok()?;
    Some(PuppetPose::from_transform_block(&block))
}

/// One log text per companion the game holds live, so the two logs side by side show how far
/// apart the two games' companions are.
pub fn live_companion_texts<M: GameMemory>(memory: &M) -> Vec<String> {
    let base = memory.module_base();
    partner_actors(memory)
        .into_iter()
        .filter(|actor| actor_is_live(memory, *actor).unwrap_or(false))
        .filter_map(|actor| {
            let block = memory
                .read_array::<ACTOR_TRANSFORM_BLOCK_BYTES>(actor + ACTOR_TRANSFORM_BLOCK_OFFSET)
                .ok()?;
            let pose = PuppetPose::from_transform_block(&block);
            let name = if actor == base + u64::from(PARTNER_KAINE_ACTOR_RVA) {
                "unique actor -2".to_owned()
            } else if actor == base + u64::from(PARTNER_THIRD_MEMBER_ACTOR_RVA) {
                "the third member".to_owned()
            } else {
                format!("variant {actor:#x}")
            };
            Some(format!(
                "{name} class {:#x} at ({:.1}, {:.1}, {:.1}) yaw {:.2}, drawn flag {:?}, {}",
                memory.read_i32(actor + ACTOR_CLASS_ID_OFFSET).unwrap_or(-1),
                pose.position.x,
                pose.position.y,
                pose.position.z,
                pose.yaw,
                memory.read_u8(actor + ACTOR_DRAWN_FLAG_OFFSET).ok(),
                motion_layers_text(memory, actor)
            ))
        })
        .collect()
}
