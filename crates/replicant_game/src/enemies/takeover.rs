use std::collections::{HashMap, HashSet};

use replicant_proto::EnemyId;

use crate::enemies::EnemySighting;

const MAX_TAKEOVER_DISTANCE: f32 = 3.0;
pub const FIGHT_SPAWN_FILL_IN_GRACE_MS: u64 = 3000;
pub const FIGHT_SPAWN_FILL_IN_RETRY_GAP_MS: u64 = 1000;
pub const FIGHT_SPAWN_FILL_IN_RETRIES: u32 = 3;

pub fn plan_takeovers(
    own_spawns: &[(EnemyId, EnemySighting)],
    copies: &[(EnemyId, EnemySighting)],
    host_live: &[EnemyId],
    already_own: &HashSet<EnemyId>,
) -> Vec<(EnemyId, EnemyId)> {
    let mut pairs: Vec<(f32, EnemyId, EnemyId)> = Vec::new();
    for (own, own_sighting) in own_spawns {
        if host_live.contains(own) {
            continue;
        }
        for (copy, copy_sighting) in copies {
            if already_own.contains(copy) {
                continue;
            }
            if copy.group == own.group
                && copy_sighting.group == own_sighting.group
                && copy_sighting.class_id == own_sighting.class_id
                && copy_sighting.actor != own_sighting.actor
            {
                let gap = distance(own_sighting.position, copy_sighting.position);
                if gap <= MAX_TAKEOVER_DISTANCE {
                    pairs.push((gap, *own, *copy));
                }
            }
        }
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut taken_own = HashSet::new();
    let mut taken_copy = HashSet::new();
    let mut plan = Vec::new();
    for (_, own, copy) in pairs {
        if taken_own.contains(&own) || taken_copy.contains(&copy) {
            continue;
        }
        taken_own.insert(own);
        taken_copy.insert(copy);
        plan.push((own, copy));
    }
    plan.sort();
    plan
}

/// Host enemies inside a fight window that the guest's replay has not spawned: each waits
/// `FIGHT_SPAWN_FILL_IN_GRACE_MS` from its arrival here for a guest spawn to bind it, then is due
/// for a copy; a copy the game would not make is tried again `FIGHT_SPAWN_FILL_IN_RETRIES` times,
/// `FIGHT_SPAWN_FILL_IN_RETRY_GAP_MS` apart.
#[derive(Debug, Clone, Default)]
pub struct FightSpawnFillIn {
    waiting_since: HashMap<EnemyId, u64>,
    filled_in: HashSet<EnemyId>,
    failed_tries: HashMap<EnemyId, (u32, u64)>,
    snap_off_logged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FightFillInStep {
    /// Host ids that just arrived with no enemy here holding them.
    pub newly_waiting: Vec<EnemyId>,
    /// (host id, ms waited): no guest spawn bound it in time, so a copy goes in now.
    pub due: Vec<(EnemyId, u64)>,
}

impl FightSpawnFillIn {
    /// One guest tick inside a fight window: `host_live` are the host's enemies, `held_here` the
    /// ids an enemy here holds, `ended` the ids killed or gone, `copyable` the ids a copy can be
    /// made of (creation known, not a boss).
    pub fn observe(
        &mut self,
        now_ms: u64,
        host_live: &[EnemyId],
        held_here: &HashSet<EnemyId>,
        ended: &HashSet<EnemyId>,
        copyable: &HashSet<EnemyId>,
    ) -> FightFillInStep {
        let waiting: HashSet<EnemyId> = host_live
            .iter()
            .filter(|id| !held_here.contains(id) && !ended.contains(id))
            .copied()
            .collect();
        self.waiting_since.retain(|id, _| waiting.contains(id));
        let mut step = FightFillInStep::default();
        let mut ids: Vec<EnemyId> = waiting.into_iter().collect();
        ids.sort();
        for id in ids {
            if self.filled_in.contains(&id) {
                continue;
            }
            let since = *self.waiting_since.entry(id).or_insert_with(|| {
                step.newly_waiting.push(id);
                now_ms
            });
            let waited = now_ms.saturating_sub(since);
            let retry_ready = self.failed_tries.get(&id).is_none_or(|&(tries, last_ms)| {
                tries <= FIGHT_SPAWN_FILL_IN_RETRIES
                    && now_ms.saturating_sub(last_ms) >= FIGHT_SPAWN_FILL_IN_RETRY_GAP_MS
            });
            if waited >= FIGHT_SPAWN_FILL_IN_GRACE_MS && copyable.contains(&id) && retry_ready {
                step.due.push((id, waited));
            }
        }
        step
    }

    /// The copy for `id` was made: it is never due again in this window.
    pub fn mark_filled_in(&mut self, id: EnemyId) {
        self.waiting_since.remove(&id);
        self.failed_tries.remove(&id);
        self.filled_in.insert(id);
    }

    /// The game would not make the copy for `id`. Returns the number of tries when that was the
    /// last one, so the caller logs the give-up once.
    pub fn copy_failed(&mut self, id: EnemyId, now_ms: u64) -> Option<u32> {
        let entry = self.failed_tries.entry(id).or_insert((0, now_ms));
        entry.0 = entry.0.saturating_add(1);
        entry.1 = now_ms;
        (entry.0 > FIGHT_SPAWN_FILL_IN_RETRIES).then_some(entry.0)
    }

    /// True once a copy for `id` was tried here in this window and the game would not make it.
    pub fn tried(&self, id: EnemyId) -> bool {
        self.failed_tries.contains_key(&id)
    }

    /// A fill-in was due but snap (the guest's authority over copies) is off. True only the first
    /// time in a window, so the skip is logged once.
    pub fn first_skip_with_snap_off(&mut self) -> bool {
        !std::mem::replace(&mut self.snap_off_logged, true)
    }

    pub fn clear(&mut self) {
        self.waiting_since.clear();
        self.filled_in.clear();
        self.failed_tries.clear();
        self.snap_off_logged = false;
    }
}

pub fn fight_spawn_filled_in_text(id: EnemyId, waited_ms: u64) -> String {
    format!(
        "fight spawn filled in {}:{} after {waited_ms}",
        id.group, id.ordinal
    )
}

pub fn fight_spawn_fill_in_gave_up_text(id: EnemyId, tries: u32) -> String {
    format!(
        "fight spawn fill-in gave up on {}:{} after {tries} tries: the game would not create the copy",
        id.group, id.ordinal
    )
}

pub fn fight_spawn_fill_in_snap_off_text(id: EnemyId) -> String {
    format!(
        "fight spawn fill-in due for {}:{} but snap is off, so no copy is made (logged once per fight window)",
        id.group, id.ordinal
    )
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1]).hypot(a[2] - b[2])
}
