use std::collections::{HashMap, HashSet};

use replicant_game::GameMemory;
use replicant_game::enemies::{
    AggroPlanner, CopyStanceLog, CreationWatch, DamageTakenLedger, DyingBodies, GuestHitGuard,
    GuestOnlyWatch, HostHitStaggerWatch, KillWatch, PlayerHitGuard, RemovalCause,
};
use replicant_game::offsets::{ACTOR_KIND_MASK_OFFSET, ACTOR_SPAWN_SERIAL_OFFSET};
use replicant_game::puppet::actor_is_live;
use replicant_proto::{EnemyCreation, EnemyDrop, EnemyId, EnemyKilled, EnemyWordDrop, PlayerHit};

use crate::enemies::counters::CombatCounts;
use crate::memory::InProcessMemory;

/// Everything the shared combat remembers between ticks, on either side.
#[derive(Debug, Default)]
pub(super) struct CombatState {
    pub(super) phase: Option<u16>,
    pub(super) actors: HashMap<EnemyId, LocalEnemy>,
    pub(super) aggro: AggroPlanner,
    pub(super) kills: KillWatch,
    pub(super) guard: GuestHitGuard,
    pub(super) player_hit_guard: PlayerHitGuard,
    pub(super) guest_kills: HashSet<EnemyId>,
    pub(super) puppet_kind: Option<PuppetKind>,
    pub(super) killed_copies: HashSet<u64>,
    pub(super) pending_kills: Vec<(EnemyKilled, u64)>,
    pub(super) pending_player_hits: Vec<PlayerHit>,
    pub(super) pending_drops: Vec<EnemyDrop>,
    pub(super) pending_word_drops: Vec<EnemyWordDrop>,
    pub(super) pending_removals: Vec<(u16, EnemyId)>,
    pub(super) ended_ids: HashSet<EnemyId>,
    pub(super) dying_bodies: HashMap<u64, (EnemyId, u64, i32)>,
    pub(super) pending_body_gone: Vec<(u16, EnemyId)>,
    pub(super) host_gone_bodies: HashSet<EnemyId>,
    pub(super) stances: CopyStanceLog,
    pub(super) menu_hit_skip_logged_ms: Option<u64>,
    pub(super) stagger_watch: HostHitStaggerWatch,
    pub(super) aggro_menus_logged: (bool, bool),
    pub(super) host_bodies: DyingBodies,
    pub(super) motion_watch: HashMap<EnemyId, MotionWatch>,
    pub(super) last_repeat_logged: Option<(EnemyId, u32)>,
    pub(super) last_started: HashMap<EnemyId, i32>,
    pub(super) damage_taken: DamageTakenLedger,
    pub(super) announced_difficulty: Option<(u16, u16)>,
    pub(super) own_script_villagers: HashSet<u64>,
    pub(super) unreadable_motion_actors: HashSet<u64>,
    pub(super) guest_only: GuestOnlyWatch,
    pub(super) ghosts: HashMap<u64, GhostCopy>,
    pub(super) host_ended_groups: HashSet<u16>,
    pub(super) removal_causes: Vec<(EnemyId, RemovalCause)>,
    pub(super) creations: HashMap<EnemyId, EnemyCreation>,
    pub(super) created: HashSet<EnemyId>,
    pub(super) taken_over: HashSet<EnemyId>,
    pub(super) creation_watch: CreationWatch,
    pub(super) counts: CombatCounts,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct LocalEnemy {
    pub(super) actor: u64,
    pub(super) serial: i32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GhostCopy {
    pub(super) id: EnemyId,
    pub(super) serial: i32,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct MotionWatch {
    pub(super) frame: f32,
    pub(super) since_ms: u64,
    pub(super) restarted_at_ms: Option<u64>,
    pub(super) reported: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PuppetKind {
    pub(super) actor: u64,
    pub(super) original: i32,
}

impl CombatState {
    pub(super) fn reset(&mut self) {
        let counts = self.counts;
        *self = Self::default();
        self.counts = counts;
    }

    pub(super) fn enter_phase(&mut self, phase: u16) {
        if self.phase != Some(phase) {
            let mut creations = std::mem::take(&mut self.creations);
            creations.retain(|_, creation| creation.phase == phase);
            let damage_taken = std::mem::take(&mut self.damage_taken);
            self.reset();
            self.creations = creations;
            self.damage_taken = damage_taken;
            self.phase = Some(phase);
        }
    }

    pub(super) fn remember(&mut self, seen: impl IntoIterator<Item = (EnemyId, u64, i32)>) {
        self.actors = seen
            .into_iter()
            .map(|(id, actor, serial)| (id, LocalEnemy { actor, serial }))
            .collect();
    }

    pub(super) fn actor_of(&self, memory: &InProcessMemory, id: EnemyId) -> Option<u64> {
        let enemy = self.actors.get(&id)?;
        let still_it = actor_is_live(memory, enemy.actor).unwrap_or(false)
            && memory
                .read_i32(enemy.actor + ACTOR_SPAWN_SERIAL_OFFSET)
                .is_ok_and(|serial| serial == enemy.serial);
        still_it.then_some(enemy.actor)
    }

    pub(super) fn actor_ids(&self) -> HashMap<u64, EnemyId> {
        self.actors
            .iter()
            .map(|(id, enemy)| (enemy.actor, *id))
            .collect()
    }
}

pub(super) fn read_kind(memory: &InProcessMemory, actor: u64) -> Option<i32> {
    memory.read_i32(actor + ACTOR_KIND_MASK_OFFSET).ok()
}
