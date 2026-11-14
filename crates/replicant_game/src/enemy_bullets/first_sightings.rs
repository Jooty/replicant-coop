use std::collections::VecDeque;

use crate::enemy_bullets::MAX_BULLETS_REMEMBERED_FOR_LOG;

/// Remembers the last bullets already logged, so a line is written once per bullet and not
/// once per frame.
#[derive(Debug, Clone, Default)]
pub struct BulletFirstSightings {
    seen: VecDeque<u64>,
}

impl BulletFirstSightings {
    /// True the first time `bullet` (its own handle) is offered while it is remembered.
    pub fn first_sight(&mut self, bullet: u64) -> bool {
        if self.seen.contains(&bullet) {
            return false;
        }
        if self.seen.len() >= MAX_BULLETS_REMEMBERED_FOR_LOG {
            self.seen.pop_front();
        }
        self.seen.push_back(bullet);
        true
    }
}
