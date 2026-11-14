use replicant_proto::GUEST_BODY;

use crate::puppet::body_info;
use crate::stay_together::FollowRole;

pub const SEED_NAME_TABLE_STRIDE: u64 = 0x10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnSeedSwap {
    pub from: &'static str,
    pub to: &'static str,
    pub table_index: u32,
    pub changes: &'static str,
}

/// The seed the guest's phase load should use instead of `returned_seed`, or None to keep it.
/// Only the seeds the swapped body's row lists are replaced, so a seed the body already is, or one
/// a phase forces that the row does not name, stays as the game picked it.
pub fn own_seed_swap(role: FollowRole, returned_seed: &str) -> Option<OwnSeedSwap> {
    if role != FollowRole::Guest {
        return None;
    }
    let wanted = body_info(GUEST_BODY);
    let from = wanted
        .own_seed_replaces
        .iter()
        .find(|seed| **seed == returned_seed)?;
    Some(OwnSeedSwap {
        from,
        to: wanted.own_seed_name,
        table_index: wanted.own_seed_table_index,
        changes: wanted.own_body_changes,
    })
}
