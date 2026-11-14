use crate::carts::{
    CART_NEWER_MOTION_FLAG_OFFSET, CART_NEWER_MOTION_LAYER_COUNT_OFFSET,
    CART_NEWER_MOTION_LAYERS_OFFSET, CART_OLD_MOTION_LAYER_COUNT_OFFSET,
    CART_OLD_MOTION_LAYERS_OFFSET, CART_RUN_END_FRAME_OFFSET, CART_RUN_END_FRAME_WHEN_UNREAD,
    MOTION_LAYER_SLOT_INDEX_OFFSET, MOTION_LAYER_SLOTS_OFFSET, MOTION_SLOT_FRAME_OFFSET,
    NEWER_MOTION_SLOT_BYTES, NEWER_MOTION_SLOT_RATE_OFFSET, OLD_MOTION_SLOT_BYTES,
    OLD_MOTION_SLOT_RATE_OFFSET,
};
use crate::memory::GameMemory;

/// Motion layer 0 of a cart, which carries a running cart along its rail: `layer` is the layer
/// object the game's own frame setter takes, `newer` says which motion code the actor uses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CartMotion {
    pub layer: u64,
    pub newer: bool,
    pub frame: f32,
    pub rate: f32,
}

/// Reads layer 0 the way `CActor::SetMotionRate` finds it; `None` when the layer is missing or
/// plays no motion.
pub fn read_cart_motion<M: GameMemory>(memory: &M, cart: u64) -> Option<CartMotion> {
    let newer = memory.read_u8(cart + CART_NEWER_MOTION_FLAG_OFFSET).ok()? != 0;
    let (count_offset, layers_offset, slot_bytes, rate_offset) = if newer {
        (
            CART_NEWER_MOTION_LAYER_COUNT_OFFSET,
            CART_NEWER_MOTION_LAYERS_OFFSET,
            NEWER_MOTION_SLOT_BYTES,
            NEWER_MOTION_SLOT_RATE_OFFSET,
        )
    } else {
        (
            CART_OLD_MOTION_LAYER_COUNT_OFFSET,
            CART_OLD_MOTION_LAYERS_OFFSET,
            OLD_MOTION_SLOT_BYTES,
            OLD_MOTION_SLOT_RATE_OFFSET,
        )
    };
    if memory.read_i32(cart + count_offset).ok()? < 1 {
        return None;
    }
    let layer = memory.read_u64(cart + layers_offset).ok()?;
    if layer == 0 || memory.read_u64(layer).ok()? == 0 {
        return None;
    }
    let index = u64::try_from(
        memory
            .read_i32(layer + MOTION_LAYER_SLOT_INDEX_OFFSET)
            .ok()?,
    )
    .ok()?;
    let slots = memory.read_u64(layer + MOTION_LAYER_SLOTS_OFFSET).ok()?;
    if slots == 0 {
        return None;
    }
    let slot = slots + index * slot_bytes;
    // A negative motion id is an empty slot: the frame setters skip it.
    if memory.read_i32(slot).ok()? < 0 {
        return None;
    }
    Some(CartMotion {
        layer,
        newer,
        frame: memory.read_f32(slot + MOTION_SLOT_FRAME_OFFSET).ok()?,
        rate: memory.read_f32(slot + rate_offset).ok()?,
    })
}

/// The frame a run passes to end (`CActorTruck` +0x14D80, an int the actor factory sets).
pub fn read_cart_run_end_frame<M: GameMemory>(memory: &M, cart: u64) -> f32 {
    match memory.read_i32(cart + CART_RUN_END_FRAME_OFFSET) {
        Ok(frame) if frame > 0 => frame as f32,
        _ => CART_RUN_END_FRAME_WHEN_UNREAD,
    }
}
