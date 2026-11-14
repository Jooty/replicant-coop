use std::collections::BTreeSet;

use replicant_proto::{EnemyId, EnemyIdList};

use crate::enemies::ENEMY_ID_LIST_STALE_MS;

#[derive(Debug, Default)]
pub struct IdListTracker {
    current: Option<Partial>,
    complete: Option<CompleteList>,
}

#[derive(Debug)]
struct Partial {
    tick: u32,
    parts: u8,
    arrived: u32,
    ids: BTreeSet<EnemyId>,
}

#[derive(Debug)]
struct CompleteList {
    at_ms: u64,
    ids: BTreeSet<EnemyId>,
}

impl IdListTracker {
    pub fn receive(&mut self, at_ms: u64, list: &EnemyIdList) {
        if let Some(current) = &self.current
            && list.tick.wrapping_sub(current.tick) > u32::MAX / 2
        {
            return;
        }
        if self
            .current
            .as_ref()
            .is_none_or(|current| current.tick != list.tick)
        {
            self.current = Some(Partial {
                tick: list.tick,
                parts: list.parts,
                arrived: 0,
                ids: BTreeSet::new(),
            });
        }
        let Some(current) = &mut self.current else {
            return;
        };
        current.arrived |= 1_u32 << u32::from(list.part);
        current.ids.extend(list.ids.iter().copied());
        if current.arrived == (1_u32 << u32::from(current.parts)) - 1 {
            self.complete = Some(CompleteList {
                at_ms,
                ids: std::mem::take(&mut current.ids),
            });
            self.current = None;
        }
    }

    pub fn note_spawned(&mut self, id: EnemyId) {
        if let Some(list) = &mut self.complete {
            list.ids.insert(id);
        }
    }

    pub fn note_gone(&mut self, id: EnemyId) {
        if let Some(list) = &mut self.complete {
            list.ids.remove(&id);
        }
    }

    pub fn fresh(&self, now_ms: u64) -> Option<&BTreeSet<EnemyId>> {
        self.complete
            .as_ref()
            .filter(|list| now_ms.saturating_sub(list.at_ms) <= ENEMY_ID_LIST_STALE_MS)
            .map(|list| &list.ids)
    }
}
