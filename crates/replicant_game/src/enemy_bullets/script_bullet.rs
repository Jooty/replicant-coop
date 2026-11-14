use replicant_proto::{
    BULLET_EFFECT_SLOTS, BULLET_STYLE_FLOATS, BULLET_TRANSFORM_FLOATS, BulletLook, BulletSprite,
};

use crate::bullet_flights::{SightedBullet, partner_effect_id};
use crate::effects::{effect_instance_address, read_anchor_offset};
use crate::enemy_bullets::SCRIPT_BULLET_LASER_FLAG_OFFSET;
use crate::memory::GameMemory;
use crate::offsets::{
    ACTOR_TRANSFORM_BLOCK_BYTES, ACTOR_TRANSFORM_BLOCK_OFFSET, BULLET_SECONDS_BEFORE_START_OFFSET,
    EFFECT_INSTANCE_MATRIX_OFFSET, EFFECT_INSTANCE_STYLE_OFFSET, NORMAL_BULLET_HIT_ID_OFFSET,
    SCRIPT_BULLET_EFFECT_OBJECT_EFFECT_ID_OFFSET, SCRIPT_BULLET_EFFECT_OBJECT_HANDLE_OFFSET,
    SCRIPT_BULLET_EFFECT_OBJECT_POINTER_OFFSET, SCRIPT_BULLET_LOOP_EFFECT_VTABLE_RVA,
    SCRIPT_BULLET_SPRITE_ALPHA_OFFSET, SCRIPT_BULLET_SPRITE_ANGLE_RADIANS_OFFSET,
    SCRIPT_BULLET_SPRITE_EFFECT_VTABLE_RVA, SCRIPT_BULLET_SPRITE_KIND_OFFSET,
    SCRIPT_BULLET_SPRITE_SIZE_OFFSET, SCRIPT_BULLET_TO_SENIOR_EFFECT_VTABLE_RVA,
};

const FLOAT_BYTES: usize = 4;
const STYLE_BYTES: usize = BULLET_STYLE_FLOATS * FLOAT_BYTES;
const TRANSFORM_BYTES: usize = BULLET_TRANSFORM_FLOATS * FLOAT_BYTES;
const TRANSFORM_ROW_FLOATS: usize = 4;
const TRANSLATION_COLUMN: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotCopyable {
    SpriteNotDrawn,
    NoEffect,
}

/// Why a script bullet with no effect to copy cannot be shown, read again for the host's log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UncopiedWhy {
    NoEffectObject,
    UnknownEffectObject { vtable_rva: u32 },
    EffectIdOutOfRange { effect_id: i32 },
    EffectGone { effect_id: i32 },
}

/// A script bullet is drawn by one effect object: a looping effect made by id, shown again from
/// that id at the place the effect is, or a sprite the game adds to its sprite batch each frame,
/// sighted at the bullet's own place with what the batch needs.
pub fn read_script_bullet<M: GameMemory>(
    memory: &M,
    actor: u64,
    slot: u16,
) -> Result<(SightedBullet, Option<BulletSprite>), NotCopyable> {
    let object = memory
        .read_u64(actor + SCRIPT_BULLET_EFFECT_OBJECT_POINTER_OFFSET)
        .ok()
        .filter(|object| *object != 0)
        .ok_or(NotCopyable::NoEffect)?;
    let class = memory.read_u64(object).map_err(|_| NotCopyable::NoEffect)?;
    let base = memory.module_base();
    if class == base + u64::from(SCRIPT_BULLET_SPRITE_EFFECT_VTABLE_RVA) {
        return read_sprite(memory, actor, object, slot)
            .map(|(bullet, sprite)| (bullet, Some(sprite)))
            .ok_or(NotCopyable::SpriteNotDrawn);
    }
    if class != base + u64::from(SCRIPT_BULLET_LOOP_EFFECT_VTABLE_RVA)
        && class != base + u64::from(SCRIPT_BULLET_TO_SENIOR_EFFECT_VTABLE_RVA)
    {
        return Err(NotCopyable::NoEffect);
    }
    read_looping_effect(memory, actor, object, slot)
        .map(|bullet| (bullet, None))
        .ok_or(NotCopyable::NoEffect)
}

/// The reason `read_script_bullet` found nothing to copy, for a bullet it refused with `NoEffect`.
pub fn why_not_copied<M: GameMemory>(memory: &M, actor: u64) -> UncopiedWhy {
    let Some(object) = memory
        .read_u64(actor + SCRIPT_BULLET_EFFECT_OBJECT_POINTER_OFFSET)
        .ok()
        .filter(|object| *object != 0)
    else {
        return UncopiedWhy::NoEffectObject;
    };
    let class = memory.read_u64(object).unwrap_or(0);
    let base = memory.module_base();
    if class != base + u64::from(SCRIPT_BULLET_LOOP_EFFECT_VTABLE_RVA)
        && class != base + u64::from(SCRIPT_BULLET_TO_SENIOR_EFFECT_VTABLE_RVA)
    {
        let vtable_rva = u32::try_from(class.wrapping_sub(base)).unwrap_or(u32::MAX);
        return UncopiedWhy::UnknownEffectObject { vtable_rva };
    }
    let effect_id = memory
        .read_i32(object + SCRIPT_BULLET_EFFECT_OBJECT_EFFECT_ID_OFFSET)
        .unwrap_or(-1);
    if partner_effect_id(effect_id).is_none() {
        UncopiedWhy::EffectIdOutOfRange { effect_id }
    } else {
        UncopiedWhy::EffectGone { effect_id }
    }
}

/// The game marks a script bullet spawned from the bullet script named `laser` with a zero
/// byte the bullet base sets to one.
pub fn script_bullet_is_laser<M: GameMemory>(memory: &M, actor: u64) -> bool {
    memory
        .read_u8(actor + SCRIPT_BULLET_LASER_FLAG_OFFSET)
        .is_ok_and(|flag| flag == 0)
}

fn read_sprite<M: GameMemory>(
    memory: &M,
    actor: u64,
    object: u64,
    slot: u16,
) -> Option<(SightedBullet, BulletSprite)> {
    // The game draws no bullet that still waits for its start, and no sprite without a size.
    let seconds_before_start = memory
        .read_f32(actor + BULLET_SECONDS_BEFORE_START_OFFSET)
        .ok()?;
    let sprite = BulletSprite {
        kind: memory
            .read_u8(object + SCRIPT_BULLET_SPRITE_KIND_OFFSET)
            .ok()?,
        size: memory
            .read_f32(object + SCRIPT_BULLET_SPRITE_SIZE_OFFSET)
            .ok()?,
        alpha: memory
            .read_f32(object + SCRIPT_BULLET_SPRITE_ALPHA_OFFSET)
            .ok()?,
        angle: memory
            .read_f32(object + SCRIPT_BULLET_SPRITE_ANGLE_RADIANS_OFFSET)
            .ok()?,
    };
    if seconds_before_start > 0.0 || sprite.size <= 0.0 || sprite.validate().is_err() {
        return None;
    }
    let block = memory
        .read_array::<ACTOR_TRANSFORM_BLOCK_BYTES>(actor + ACTOR_TRANSFORM_BLOCK_OFFSET)
        .ok()?;
    let mut transform: [f32; BULLET_TRANSFORM_FLOATS] = floats_of(&block);
    let anchor = read_anchor_offset(memory, actor).ok()?;
    // The sprite sits at the bullet's matrix applied to its anchor offset.
    for row in transform.as_chunks_mut::<TRANSFORM_ROW_FLOATS>().0 {
        row[TRANSLATION_COLUMN] += row[0] * anchor[0] + row[1] * anchor[1] + row[2] * anchor[2];
    }
    let bullet = SightedBullet {
        actor,
        slot,
        hit_id: memory.read_i32(actor + NORMAL_BULLET_HIT_ID_OFFSET).ok()?,
        transform,
        look: BulletLook {
            effect_ids: [0; BULLET_EFFECT_SLOTS],
            styles: [[0.0; BULLET_STYLE_FLOATS]; BULLET_EFFECT_SLOTS],
        },
    };
    Some((bullet, sprite))
}

fn read_looping_effect<M: GameMemory>(
    memory: &M,
    actor: u64,
    object: u64,
    slot: u16,
) -> Option<SightedBullet> {
    let effect_id = memory
        .read_i32(object + SCRIPT_BULLET_EFFECT_OBJECT_EFFECT_ID_OFFSET)
        .ok()?;
    partner_effect_id(effect_id)?;
    let handle = memory
        .read_i32(object + SCRIPT_BULLET_EFFECT_OBJECT_HANDLE_OFFSET)
        .ok()?
        .cast_unsigned();
    let instance = effect_instance_address(memory, handle).ok().flatten()?;
    let hit_id = memory.read_i32(actor + NORMAL_BULLET_HIT_ID_OFFSET).ok()?;
    let transform = memory
        .read_array::<TRANSFORM_BYTES>(instance + EFFECT_INSTANCE_MATRIX_OFFSET)
        .ok()?;
    let style = memory
        .read_array::<STYLE_BYTES>(instance + EFFECT_INSTANCE_STYLE_OFFSET)
        .ok()?;
    let mut effect_ids = [0_i32; BULLET_EFFECT_SLOTS];
    let mut styles = [[0.0_f32; BULLET_STYLE_FLOATS]; BULLET_EFFECT_SLOTS];
    effect_ids[0] = effect_id;
    styles[0] = floats_of(&style);
    Some(SightedBullet {
        actor,
        slot,
        hit_id,
        transform: floats_of(&transform),
        look: BulletLook { effect_ids, styles },
    })
}

fn floats_of<const FLOATS: usize>(bytes: &[u8]) -> [f32; FLOATS] {
    core::array::from_fn(|index| {
        let at = index * FLOAT_BYTES;
        f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
    })
}
