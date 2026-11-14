use core::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};

use replicant_game::GameMemory;
use replicant_game::door_sync::{
    DOOR_LOCK_BYTE_OFFSET, DOOR_OPEN_BYTE_OFFSET, DOOR_VTABLE_RVA,
    MAP_ACTOR_BY_LAYOUT_ID_FUNCTION_RVA, MAP_ACTOR_TABLE_POINTER_OFFSET,
};
use replicant_game::offsets::{
    DOOR_MESSAGE_FUNCTION_RVA, MAP_LAYOUT_ID_OF_ACTOR_FUNCTION_RVA, MAP_MANAGER_RVA,
};

use crate::build_gate::VerifiedBuild;
use crate::memory::InProcessMemory;

static APPLYING_FROM_NETWORK: AtomicBool = AtomicBool::new(false);

type DoorMessage = unsafe extern "system" fn(*mut c_void, u32, u64, i64, i32) -> u64;
type LayoutIdOfActor = unsafe extern "system" fn(*mut c_void, *mut c_void) -> i32;
type ActorOfLayoutId = unsafe extern "system" fn(*mut c_void, i32) -> *mut c_void;

pub(super) fn applying_from_network() -> bool {
    APPLYING_FROM_NETWORK.load(Ordering::Acquire)
}

#[expect(
    unsafe_code,
    reason = "calls the game's map manager lookup of an actor's layout id"
)]
pub fn layout_id_of(build: &VerifiedBuild, door: *mut c_void) -> i32 {
    let base = build.module_base();
    let manager = (base + u64::from(MAP_MANAGER_RVA)) as *mut c_void;
    let lookup = (base + u64::from(MAP_LAYOUT_ID_OF_ACTOR_FUNCTION_RVA)) as *const ();
    // SAFETY: the RVA starts the map manager's actor to layout id lookup in the verified build; it
    // only walks the manager's actor lists and returns -1 when the actor is not among them.
    unsafe { core::mem::transmute::<*const (), LayoutIdOfActor>(lookup)(manager, door) }
}

#[expect(
    unsafe_code,
    reason = "calls the game's map manager lookup of an actor by layout id"
)]
pub fn actor_of_layout_id(build: &VerifiedBuild, layout_id: u16) -> Option<*mut c_void> {
    let base = build.module_base();
    let manager = base + u64::from(MAP_MANAGER_RVA);
    let memory = InProcessMemory::new(build);
    if memory
        .read_u64(manager + MAP_ACTOR_TABLE_POINTER_OFFSET)
        .ok()?
        == 0
    {
        return None;
    }
    let lookup = (base + u64::from(MAP_ACTOR_BY_LAYOUT_ID_FUNCTION_RVA)) as *const ();
    // SAFETY: the RVA starts the map manager's layout id to actor lookup in the verified build;
    // the actor table it walks was just checked to exist, and it returns null for an unknown id.
    let actor = unsafe {
        core::mem::transmute::<*const (), ActorOfLayoutId>(lookup)(
            manager as *mut c_void,
            i32::from(layout_id),
        )
    };
    (!actor.is_null()).then_some(actor)
}

pub(super) fn door_of_layout_id(build: &VerifiedBuild, layout_id: u16) -> Option<*mut c_void> {
    let actor = actor_of_layout_id(build, layout_id)?;
    let vtable = InProcessMemory::new(build).read_u64(actor as u64).ok()?;
    (vtable == build.module_base() + u64::from(DOOR_VTABLE_RVA)).then_some(actor)
}

pub(super) fn read_door_state(build: &VerifiedBuild, door: *mut c_void) -> Option<(bool, bool)> {
    let memory = InProcessMemory::new(build);
    let open = memory.read_u8(door as u64 + DOOR_OPEN_BYTE_OFFSET).ok()?;
    let locked = memory.read_u8(door as u64 + DOOR_LOCK_BYTE_OFFSET).ok()?;
    Some((open != 0, locked != 0))
}

#[expect(
    unsafe_code,
    reason = "calls the door actor's message method, hooked or not, by its address"
)]
pub(super) fn send_door_message(
    build: &VerifiedBuild,
    door: *mut c_void,
    message: u32,
    argument: i64,
) {
    let function = (build.module_base() + u64::from(DOOR_MESSAGE_FUNCTION_RVA)) as *const ();
    APPLYING_FROM_NETWORK.store(true, Ordering::Release);
    // SAFETY: the RVA starts CActorDoor's message method in the verified build; the door was just
    // looked up and checked against the door vtable; this runs on the game thread.
    unsafe {
        core::mem::transmute::<*const (), DoorMessage>(function)(door, message, 0, argument, 0);
    }
    APPLYING_FROM_NETWORK.store(false, Ordering::Release);
}
