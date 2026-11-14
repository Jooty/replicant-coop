use std::collections::{BTreeMap, BTreeSet};

use replicant_proto::{
    EnemyGone, EnemyId, EnemyIdList, EnemySnapshot, EnemySpawned, EnemyState, MotionState,
    PartnerActivity, PlayerState, StateTrack,
};

use crate::enemies::CompleteRound;
use crate::enemies::host_id_list::IdListTracker;
use crate::enemies::host_rounds::RoundTracker;
use crate::enemies::{ENEMY_MIRROR_STALE_MS, HostListView};

/// The guest's picture of the host's enemies, built from snapshots and spawn and death events,
/// with each enemy's motion buffered so it can be replayed a little behind, like the ghost.
#[derive(Debug, Default)]
pub struct EnemyMirror {
    phase: Option<u16>,
    host_total: Option<u16>,
    rounds: RoundTracker,
    id_lists: IdListTracker,
    last_tick: u32,
    enemies: BTreeMap<EnemyId, MirroredTrack>,
    gone: BTreeSet<EnemyId>,
}

#[derive(Debug)]
struct MirroredTrack {
    track: StateTrack,
    latest: EnemyState,
    seen_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MirroredEnemy {
    pub id: EnemyId,
    pub position: [f32; 3],
    pub yaw_radians: f32,
    pub motion: Option<MotionState>,
    pub hp: u32,
    pub max_hp: u32,
}

impl EnemyMirror {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn phase(&self) -> Option<u16> {
        self.phase
    }

    pub fn host_total(&self) -> Option<u16> {
        self.host_total
    }

    pub fn latest_complete_round(&self) -> Option<CompleteRound> {
        self.rounds.latest_complete()
    }

    pub fn host_view(&self, now_ms: u64) -> HostListView {
        let round_covers_host = self.rounds.covers_every_host_enemy(now_ms);
        HostListView {
            round_covers_host,
            listed: if round_covers_host {
                None
            } else {
                self.id_lists.fresh(now_ms).cloned()
            },
        }
    }

    pub fn len(&self) -> usize {
        self.enemies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.enemies.is_empty()
    }

    pub fn receive_snapshot(&mut self, at_ms: u64, snapshot: &EnemySnapshot) {
        self.enter_phase(snapshot.phase);
        self.host_total = Some(snapshot.total);
        self.rounds.receive(at_ms, snapshot);
        self.last_tick = snapshot.tick;
        for enemy in &snapshot.enemies {
            self.update(at_ms, snapshot.tick, *enemy);
        }
    }

    pub fn receive_id_list(&mut self, at_ms: u64, list: &EnemyIdList) {
        self.enter_phase(list.phase);
        self.id_lists.receive(at_ms, list);
    }

    pub fn receive_spawned(&mut self, at_ms: u64, spawned: &EnemySpawned) {
        self.enter_phase(spawned.phase);
        self.id_lists.note_spawned(spawned.enemy.id);
        self.update(at_ms, self.last_tick, spawned.enemy);
    }

    pub fn receive_gone(&mut self, gone: &EnemyGone) {
        self.enter_phase(gone.phase);
        self.enemies.remove(&gone.id);
        self.id_lists.note_gone(gone.id);
        self.gone.insert(gone.id);
    }

    pub fn prune(&mut self, now_ms: u64) {
        self.enemies
            .retain(|_, enemy| now_ms.saturating_sub(enemy.seen_at_ms) <= ENEMY_MIRROR_STALE_MS);
    }

    pub fn sample(&self, now_ms: u64, delay_ms: u64) -> Vec<MirroredEnemy> {
        self.enemies
            .iter()
            .filter_map(|(id, enemy)| {
                let sample = enemy.track.sample(now_ms, delay_ms)?;
                Some(MirroredEnemy {
                    id: *id,
                    position: sample.position,
                    yaw_radians: sample.yaw_radians,
                    motion: sample.motion,
                    hp: enemy.latest.hp,
                    max_hp: enemy.latest.max_hp,
                })
            })
            .collect()
    }

    fn enter_phase(&mut self, phase: u16) {
        if self.phase != Some(phase) {
            self.clear();
            self.phase = Some(phase);
        }
    }

    fn update(&mut self, at_ms: u64, tick: u32, enemy: EnemyState) {
        if self.gone.contains(&enemy.id) {
            return;
        }
        let phase = self.phase.unwrap_or_default();
        let entry = self
            .enemies
            .entry(enemy.id)
            .or_insert_with(|| MirroredTrack {
                track: StateTrack::new(),
                latest: enemy,
                seen_at_ms: at_ms,
            });
        let as_state = PlayerState {
            tick,
            phase,
            position: enemy.position,
            yaw_radians: enemy.yaw_radians,
            motion: enemy.motion,
            hp: enemy.hp,
            max_hp: enemy.max_hp,
            weapon: None,
            activity: PartnerActivity::Playing,
            mp: 0,
            max_mp: 0,
            level: 0,
            ridden_boar: None,
        };
        if entry.track.push(at_ms, as_state) {
            entry.latest = enemy;
        }
        entry.seen_at_ms = at_ms;
    }
}
