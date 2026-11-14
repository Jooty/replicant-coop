use std::collections::HashMap;

use crate::enemies::GUEST_ONLY_ENEMY_GRACE_MS;

#[derive(Debug, Default)]
pub struct GuestOnlyWatch {
    first_unmatched_ms: HashMap<u64, u64>,
}

impl GuestOnlyWatch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn expired(&mut self, now_ms: u64, unmatched_actors: &[u64]) -> Vec<u64> {
        self.expired_after(now_ms, unmatched_actors, GUEST_ONLY_ENEMY_GRACE_MS)
    }

    /// The actors unmatched for `grace_ms` or longer; each is returned once.
    pub fn expired_after(
        &mut self,
        now_ms: u64,
        unmatched_actors: &[u64],
        grace_ms: u64,
    ) -> Vec<u64> {
        self.first_unmatched_ms
            .retain(|actor, _| unmatched_actors.contains(actor));
        let mut expired = Vec::new();
        for actor in unmatched_actors {
            let first = *self.first_unmatched_ms.entry(*actor).or_insert(now_ms);
            if now_ms.saturating_sub(first) >= grace_ms {
                expired.push(*actor);
            }
        }
        for actor in &expired {
            self.first_unmatched_ms.remove(actor);
        }
        expired
    }
}
