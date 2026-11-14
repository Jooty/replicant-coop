use crate::bombs::{MAX_OWN_BOMB_HITS, MAX_OWN_BOMBS, OWN_BOMB_HIT_MEMORY_MS, SightedBomb};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OwnBomb {
    slot: u16,
    actor: u64,
    since_ms: u64,
}

/// What one look at the bombs changed: bombs the player took up for the first time, and own
/// bombs that are gone with how long they were shown.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OwnBombChanges {
    pub taken: Vec<SightedBomb>,
    pub gone: Vec<(u16, u64)>,
    pub live: Vec<SightedBomb>,
}

/// The bombs this game's player has held. A bomb stays the player's once lifted, through being
/// put down, thrown and lit, until its actor is gone.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OwnBombs {
    known: Vec<OwnBomb>,
}

/// Hit ids of the attacks this player's bombs made, so their hits on shared enemies count as
/// the player's own although the bomb itself is gone by then.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OwnBombHits {
    hits: Vec<(u32, u64)>,
}

impl OwnBombs {
    pub fn update(&mut self, sighted: &[SightedBomb], player: u64, now_ms: u64) -> OwnBombChanges {
        let mut changes = OwnBombChanges::default();
        let still_there = |known: &OwnBomb| {
            sighted
                .iter()
                .any(|bomb| bomb.slot == known.slot && bomb.actor == known.actor)
        };
        for known in self.known.iter().filter(|known| !still_there(known)) {
            changes
                .gone
                .push((known.slot, now_ms.saturating_sub(known.since_ms)));
        }
        self.known.retain(still_there);
        for bomb in sighted {
            let known = self.is_own(bomb.actor);
            if !known && player != 0 && bomb.holder == Some(player) {
                if self.known.len() >= MAX_OWN_BOMBS {
                    continue;
                }
                self.known.push(OwnBomb {
                    slot: bomb.slot,
                    actor: bomb.actor,
                    since_ms: now_ms,
                });
                changes.taken.push(*bomb);
            }
            if self.is_own(bomb.actor) {
                changes.live.push(*bomb);
            }
        }
        changes
    }

    pub fn is_own(&self, actor: u64) -> bool {
        actor != 0 && self.known.iter().any(|known| known.actor == actor)
    }

    pub fn take_all(&mut self, now_ms: u64) -> Vec<(u16, u64)> {
        self.known
            .drain(..)
            .map(|known| (known.slot, now_ms.saturating_sub(known.since_ms)))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.known.len()
    }

    pub fn is_empty(&self) -> bool {
        self.known.is_empty()
    }
}

impl OwnBombHits {
    pub fn note(&mut self, hit_id: u32, now_ms: u64) {
        self.forget_old(now_ms);
        if self.hits.len() >= MAX_OWN_BOMB_HITS {
            self.hits.remove(0);
        }
        self.hits.push((hit_id, now_ms));
    }

    pub fn is_own(&mut self, hit_id: u32, now_ms: u64) -> bool {
        self.forget_old(now_ms);
        hit_id != 0 && self.hits.iter().any(|(known, _)| *known == hit_id)
    }

    fn forget_old(&mut self, now_ms: u64) {
        self.hits
            .retain(|(_, at)| now_ms.saturating_sub(*at) < OWN_BOMB_HIT_MEMORY_MS);
    }
}
