use replicant_proto::WallBroken;

use crate::blocks::distance_between;
use crate::bombs::{
    MAX_PENDING_WALLS, SAME_WALL_DISTANCE, WALL_GIVE_UP_MS, WALL_RETRY_INTERVAL_MS,
};

#[derive(Debug, Clone, Copy, PartialEq)]
struct PendingWall {
    wall: WallBroken,
    first_try_ms: Option<u64>,
    last_try_ms: u64,
}

/// The partner's wall breaks this game still has to copy. A break waits while this game is in
/// another zone, is tried again until this game's wall is found, and is dropped once copied.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BrokenWalls {
    pending: Vec<PendingWall>,
}

impl BrokenWalls {
    pub fn note(&mut self, wall: WallBroken) -> bool {
        if self.pending.iter().any(|known| same_wall(known.wall, wall)) {
            return true;
        }
        if self.pending.len() >= MAX_PENDING_WALLS {
            return false;
        }
        self.pending.push(PendingWall {
            wall,
            first_try_ms: None,
            last_try_ms: 0,
        });
        true
    }

    pub fn due(&mut self, local_phase: Option<u16>, now_ms: u64) -> Vec<WallBroken> {
        let mut due = Vec::new();
        for pending in &mut self.pending {
            if Some(pending.wall.phase) != local_phase {
                continue;
            }
            let first = *pending.first_try_ms.get_or_insert(now_ms);
            if first != now_ms
                && now_ms.saturating_sub(pending.last_try_ms) < WALL_RETRY_INTERVAL_MS
            {
                continue;
            }
            pending.last_try_ms = now_ms;
            due.push(pending.wall);
        }
        due
    }

    pub fn copied(&mut self, wall: WallBroken) {
        self.pending.retain(|known| !same_wall(known.wall, wall));
    }

    pub fn take_given_up(&mut self, now_ms: u64) -> Vec<WallBroken> {
        let given_up = |pending: &PendingWall| {
            pending
                .first_try_ms
                .is_some_and(|first| now_ms.saturating_sub(first) >= WALL_GIVE_UP_MS)
        };
        let dropped = self
            .pending
            .iter()
            .filter(|pending| given_up(pending))
            .map(|pending| pending.wall)
            .collect();
        self.pending.retain(|pending| !given_up(pending));
        dropped
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

pub fn same_wall(left: WallBroken, right: WallBroken) -> bool {
    if left.phase != right.phase {
        return false;
    }
    if let (Some(left_id), Some(right_id)) = (left.layout_id, right.layout_id) {
        return left_id == right_id;
    }
    distance_between(left.position, right.position) <= SAME_WALL_DISTANCE
}
