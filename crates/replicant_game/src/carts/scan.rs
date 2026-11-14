use replicant_proto::CartRide;

use crate::carts::{CART_MATCH_UNITS, CART_STATE_OFFSET, CART_VTABLE_RVA, CartState, cart_state};
use crate::memory::GameMemory;
use crate::offsets::{
    ACTOR_LIFE_STATE_OFFSET, ACTOR_MANAGER_RVA, ACTOR_MANAGER_SECONDARY_LIST_HEAD_OFFSET,
    ACTOR_SECONDARY_LIST_NEXT_OFFSET, ACTOR_TRANSFORM_BLOCK_BYTES, ACTOR_TRANSFORM_BLOCK_OFFSET,
    MAX_ACTORS_WALKED_IN_SECONDARY_LIST,
};
use crate::puppet::{PuppetPose, actor_is_live};
use crate::stay_together::distance_between;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CartSighting {
    pub actor: u64,
    pub layout_id: Option<u16>,
    pub state: CartState,
    pub position: [f32; 3],
}

pub fn cart_is_live<M: GameMemory>(memory: &M, actor: u64) -> bool {
    actor != 0
        && memory
            .read_u64(actor)
            .is_ok_and(|vtable| vtable == memory.module_base() + u64::from(CART_VTABLE_RVA))
}

pub fn read_cart_state<M: GameMemory>(memory: &M, actor: u64) -> Option<CartState> {
    memory
        .read_i32(actor + CART_STATE_OFFSET)
        .ok()
        .map(cart_state)
}

/// Every live cart (`CActorTruck`) on the actor manager's secondary list, in list order. Only
/// memory reads; the layout id comes from the caller, which may ask the game's map manager.
pub fn scan_cart_actors<M: GameMemory>(
    memory: &M,
    mut layout_id_of: impl FnMut(u64) -> Option<u16>,
) -> Vec<CartSighting> {
    let base = memory.module_base();
    let Ok(mut actor) = memory
        .read_u64(base + u64::from(ACTOR_MANAGER_RVA) + ACTOR_MANAGER_SECONDARY_LIST_HEAD_OFFSET)
    else {
        return Vec::new();
    };
    let mut carts = Vec::new();
    for _ in 0..MAX_ACTORS_WALKED_IN_SECONDARY_LIST {
        if actor == 0 {
            break;
        }
        if cart_is_live(memory, actor)
            && let Some((state, position)) = sight(memory, actor)
        {
            carts.push(CartSighting {
                actor,
                layout_id: layout_id_of(actor),
                state,
                position,
            });
        }
        let Ok(next) = memory.read_u64(actor + ACTOR_SECONDARY_LIST_NEXT_OFFSET) else {
            break;
        };
        actor = next;
    }
    carts
}

/// The cart a partner's report names: the one with its layout id, else the nearest within
/// `CART_MATCH_UNITS` of the place it gave.
pub fn match_cart(
    carts: &[CartSighting],
    layout_id: Option<u16>,
    position: [f32; 3],
) -> Option<CartSighting> {
    if let Some(wanted) = layout_id
        && let Some(cart) = carts.iter().find(|cart| cart.layout_id == Some(wanted))
    {
        return Some(*cart);
    }
    carts
        .iter()
        .filter_map(|cart| distance_between(cart.position, position).map(|away| (away, cart)))
        .filter(|(away, _)| *away <= CART_MATCH_UNITS)
        .min_by(|(a, _), (b, _)| a.total_cmp(b))
        .map(|(_, cart)| *cart)
}

/// This game's cart for the host's ride: the one with the ride's layout id, else, among the carts
/// within `CART_MATCH_UNITS` of the host's cart actor, one parked where the host's ride started,
/// then the nearest.
pub fn match_ride_cart(carts: &[CartSighting], ride: &CartRide) -> Option<CartSighting> {
    if let Some(wanted) = ride.layout_id
        && let Some(cart) = carts.iter().find(|cart| cart.layout_id == Some(wanted))
    {
        return Some(*cart);
    }
    let start = CartState::Parked(ride.from_state);
    carts
        .iter()
        .filter_map(|cart| {
            distance_between(cart.position, ride.cart_position).map(|away| (away, cart))
        })
        .filter(|(away, _)| *away <= CART_MATCH_UNITS)
        .min_by(|(a, first), (b, second)| {
            (first.state != start)
                .cmp(&(second.state != start))
                .then(a.total_cmp(b))
        })
        .map(|(_, cart)| *cart)
}

fn sight<M: GameMemory>(memory: &M, actor: u64) -> Option<(CartState, [f32; 3])> {
    if !actor_is_live(memory, actor).ok()? {
        return None;
    }
    // The manager's own lookups skip actors whose life state is above zero (5 is destroyed).
    if memory.read_i32(actor + ACTOR_LIFE_STATE_OFFSET).ok()? > 0 {
        return None;
    }
    let block = memory
        .read_array::<ACTOR_TRANSFORM_BLOCK_BYTES>(actor + ACTOR_TRANSFORM_BLOCK_OFFSET)
        .ok()?;
    let pose = PuppetPose::from_transform_block(&block);
    Some((
        read_cart_state(memory, actor)?,
        [pose.position.x, pose.position.y, pose.position.z],
    ))
}
