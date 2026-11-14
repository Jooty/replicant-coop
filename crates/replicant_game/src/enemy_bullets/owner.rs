use crate::enemies::ActorHandle;
use crate::enemy_bullets::scan::bullet_class;
use crate::memory::GameMemory;
use crate::offsets::NORMAL_BULLET_OWNER_HANDLE_OFFSET;

/// The live actor that fired `actor`, when `actor` is an ordinary, script or trace bullet.
pub fn bullet_owner<M: GameMemory>(memory: &M, actor: u64) -> Option<u64> {
    bullet_class(memory, actor)?;
    ActorHandle::read(memory, actor + NORMAL_BULLET_OWNER_HANDLE_OFFSET)?.resolve(memory)
}
