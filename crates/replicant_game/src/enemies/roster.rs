use std::collections::{HashMap, HashSet};

use replicant_proto::{EnemyGoneReason, EnemyId};

use crate::enemies::EnemySighting;

/// Gives each enemy of the current phase its network id, (script group, spawn ordinal), the same
/// way on both machines: new enemies of a group are numbered in the order this machine spawned
/// them (its spawn serial), counting from 0 when the phase begins.
#[derive(Debug, Default)]
pub struct EnemyRoster {
    phase: Option<u16>,
    next_ordinal: HashMap<u16, u16>,
    own_arrivals: HashMap<u16, u16>,
    retired_copies: HashMap<u16, u16>,
    known: HashMap<(u64, i32), KnownEnemy>,
    last_phase_leftovers: HashSet<(u64, i32)>,
}

#[derive(Debug, Clone, Copy)]
struct KnownEnemy {
    id: EnemyId,
    hp: i32,
    own_arrival: Option<u16>,
    waits_for_host: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RosterUpdate {
    pub phase_changed: bool,
    pub unloaded: Vec<EnemyId>,
    pub seen: Vec<(EnemyId, EnemySighting)>,
    pub spawned: Vec<EnemyId>,
    pub gone: Vec<(EnemyId, EnemyGoneReason)>,
    /// Enemies of the phase just left that the game still holds: they get no id in the new phase.
    pub last_phase_leftovers_found: usize,
    pub last_phase_leftovers_gone: usize,
}

impl EnemyRoster {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn phase(&self) -> Option<u16> {
        self.phase
    }

    pub fn len(&self) -> usize {
        self.known.len()
    }

    pub fn is_empty(&self) -> bool {
        self.known.is_empty()
    }

    pub fn observe(&mut self, phase: u16, sightings: &[EnemySighting]) -> RosterUpdate {
        let mut update = RosterUpdate::default();
        if self.phase != Some(phase) {
            update.unloaded = self.known.values().map(|known| known.id).collect();
            update.unloaded.sort();
            let still_here: HashSet<(u64, i32)> = sightings
                .iter()
                .map(key_of)
                .filter(|key| {
                    self.known.contains_key(key) || self.last_phase_leftovers.contains(key)
                })
                .collect();
            self.reset();
            self.phase = Some(phase);
            update.phase_changed = true;
            update.last_phase_leftovers_found = still_here.len();
            self.last_phase_leftovers = still_here;
        } else if !self.last_phase_leftovers.is_empty() {
            let before = self.last_phase_leftovers.len();
            self.last_phase_leftovers
                .retain(|key| sightings.iter().any(|sighting| key_of(sighting) == *key));
            update.last_phase_leftovers_gone = before - self.last_phase_leftovers.len();
        }
        // The phase number can change before the game unloads the old phase's actors.
        let mut arrivals: Vec<&EnemySighting> = sightings
            .iter()
            .filter(|sighting| {
                let key = key_of(sighting);
                !self.known.contains_key(&key) && !self.last_phase_leftovers.contains(&key)
            })
            .collect();
        arrivals.sort_by_key(|sighting| (sighting.group, sighting.spawn_serial));
        arrivals.dedup_by_key(|sighting| key_of(sighting));
        for sighting in arrivals {
            let next = self.next_ordinal.entry(sighting.group).or_insert(0);
            let id = EnemyId {
                group: sighting.group,
                ordinal: *next,
            };
            *next = next.saturating_add(1);
            let arrivals = self.own_arrivals.entry(sighting.group).or_insert(0);
            let own_arrival = *arrivals;
            *arrivals = arrivals.saturating_add(1);
            self.known.insert(
                key_of(sighting),
                KnownEnemy {
                    id,
                    hp: sighting.hp,
                    own_arrival: Some(own_arrival),
                    waits_for_host: true,
                },
            );
            update.spawned.push(id);
        }
        let mut present = HashSet::with_capacity(sightings.len());
        for sighting in sightings {
            if let Some(known) = self.known.get_mut(&key_of(sighting)) {
                known.hp = sighting.hp;
                present.insert(key_of(sighting));
                update.seen.push((known.id, *sighting));
            }
        }
        self.known.retain(|key, known| {
            let stays = present.contains(key);
            if !stays {
                let reason = if known.hp <= 0 {
                    EnemyGoneReason::Killed
                } else {
                    EnemyGoneReason::Removed
                };
                update.gone.push((known.id, reason));
            }
            stays
        });
        update.seen.sort_by_key(|(id, _)| *id);
        update.seen.dedup_by_key(|(id, _)| *id);
        update.spawned.sort();
        update.gone.sort_by_key(|(id, _)| *id);
        update
    }

    pub fn claim(&mut self, actor: u64, spawn_serial: i32, id: EnemyId, hp: i32) {
        self.known.insert(
            (actor, spawn_serial),
            KnownEnemy {
                id,
                hp,
                own_arrival: None,
                waits_for_host: false,
            },
        );
        let next = self.next_ordinal.entry(id.group).or_insert(0);
        *next = (*next).max(id.ordinal.saturating_add(1));
    }

    /// On the guest: the game's own new enemy `own` takes the id of the host's created copy
    /// `copy`; the copy's entry moves to an id no host enemy has, so it can be removed.
    pub fn take_over_copy(&mut self, own: EnemyId, copy: EnemyId) -> bool {
        let holds = |id: EnemyId| {
            self.known
                .iter()
                .find(|(_, known)| known.id == id)
                .map(|(key, _)| *key)
        };
        let (Some(own_key), Some(copy_key)) = (holds(own), holds(copy)) else {
            return false;
        };
        let retired_id = self.retired_id(copy.group);
        if let Some(known) = self.known.get_mut(&copy_key) {
            known.id = retired_id;
        }
        if let Some(known) = self.known.get_mut(&own_key) {
            known.id = copy;
        }
        true
    }

    /// On the guest: a new enemy whose id the host does not have takes the lowest live host id of
    /// its group that no enemy here holds, so a boss started a second time on one game (its
    /// ordinal ran ahead) still pairs with the host's. Returns each (own id, adopted id).
    pub fn adopt_host_ids(
        &mut self,
        spawned: &[EnemyId],
        host_live: &[EnemyId],
    ) -> Vec<(EnemyId, EnemyId)> {
        let mut held: HashSet<EnemyId> = self.known.values().map(|known| known.id).collect();
        let mut adoptions = Vec::new();
        for own in spawned {
            if host_live.contains(own) {
                continue;
            }
            let Some(adopted) = host_live
                .iter()
                .filter(|host| host.group == own.group && !held.contains(host))
                .min()
                .copied()
            else {
                continue;
            };
            let Some(known) = self.known.values_mut().find(|known| known.id == *own) else {
                continue;
            };
            known.id = adopted;
            held.remove(own);
            held.insert(adopted);
            let next = self.next_ordinal.entry(own.group).or_insert(0);
            *next = (*next).max(adopted.ordinal.saturating_add(1));
            adoptions.push((*own, adopted));
        }
        adoptions
    }

    /// How many enemies of its group this game itself had spawned in the phase before `id`'s
    /// enemy (claimed copies do not count); None for a copy made from the host's spawn.
    pub fn own_arrival_of(&self, id: EnemyId) -> Option<u16> {
        self.known
            .values()
            .find(|known| known.id == id)
            .and_then(|known| known.own_arrival)
    }

    pub fn held_ids(&self) -> HashSet<EnemyId> {
        self.known.values().map(|known| known.id).collect()
    }

    /// A fresh id in `group` that no host enemy has: ordinals count down from `u16::MAX`.
    pub fn retired_id(&mut self, group: u16) -> EnemyId {
        let retired = self.retired_copies.entry(group).or_insert(0);
        let id = EnemyId {
            group,
            ordinal: u16::MAX.saturating_sub(*retired),
        };
        *retired = retired.saturating_add(1);
        id
    }

    /// Own spawns not yet paired with a host enemy inside a fight window, oldest first.
    pub fn fight_waiting_spawns(&self) -> Vec<EnemyId> {
        let mut waiting: Vec<(u16, u16, EnemyId)> = self
            .known
            .values()
            .filter(|known| known.waits_for_host)
            .filter_map(|known| Some((known.id.group, known.own_arrival?, known.id)))
            .collect();
        waiting.sort();
        waiting.into_iter().map(|(_, _, id)| id).collect()
    }

    pub fn stop_waiting(&mut self, id: EnemyId) {
        for known in self.known.values_mut().filter(|known| known.id == id) {
            known.waits_for_host = false;
        }
    }

    pub fn stop_all_waiting(&mut self) {
        for known in self.known.values_mut() {
            known.waits_for_host = false;
        }
    }

    /// Applies every (from, to) at once, so a chain or a swap of ids works. A move is skipped
    /// when no enemy holds `from`, another move has the same `to`, or `to` stays held by an enemy
    /// that does not move; returns the moves made.
    pub fn reassign_ids(&mut self, moves: &[(EnemyId, EnemyId)]) -> Vec<(EnemyId, EnemyId)> {
        let mut targets = HashSet::new();
        let mut accepted: Vec<((u64, i32), EnemyId, EnemyId)> = Vec::new();
        for (from, to) in moves {
            let Some(key) = self
                .known
                .iter()
                .find(|(_, known)| known.id == *from)
                .map(|(key, _)| *key)
            else {
                continue;
            };
            if targets.insert(*to) {
                accepted.push((key, *from, *to));
            }
        }
        loop {
            let moving: HashSet<EnemyId> = accepted.iter().map(|(_, from, _)| *from).collect();
            let before = accepted.len();
            accepted.retain(|(_, _, to)| {
                !self
                    .known
                    .values()
                    .any(|known| known.id == *to && !moving.contains(&known.id))
            });
            if accepted.len() == before {
                break;
            }
        }
        for (key, _, to) in &accepted {
            if let Some(known) = self.known.get_mut(key) {
                known.id = *to;
            }
        }
        accepted
            .into_iter()
            .map(|(_, from, to)| (from, to))
            .collect()
    }
}

impl RosterUpdate {
    pub fn rename(&mut self, adoptions: &[(EnemyId, EnemyId)]) {
        let renamed = |id: &mut EnemyId| {
            if let Some((_, adopted)) = adoptions.iter().find(|(own, _)| own == id) {
                *id = *adopted;
            }
        };
        self.spawned.iter_mut().for_each(renamed);
        self.seen.iter_mut().for_each(|(id, _)| renamed(id));
        self.spawned.sort();
        self.seen.sort_by_key(|(id, _)| *id);
    }
}

fn key_of(sighting: &EnemySighting) -> (u64, i32) {
    (sighting.actor, sighting.spawn_serial)
}
