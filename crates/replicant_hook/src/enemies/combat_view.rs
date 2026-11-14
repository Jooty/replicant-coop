use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};
use std::sync::{LazyLock, Mutex, PoisonError};

use replicant_game::enemies::{AggroTarget, HitReaction};
use replicant_proto::EnemyId;

use crate::enemies::EnemySyncRole;

static VIEW: LazyLock<Mutex<CombatView>> = LazyLock::new(Mutex::default);

thread_local! {
    static APPLYING_REMOTE: Cell<bool> = const { Cell::new(false) };
    static PENDING_HIT_KIND: Cell<Option<(i32, Option<i32>)>> = const { Cell::new(None) };
    static PENDING_HIT_REACTION: Cell<Option<HitReaction>> = const { Cell::new(None) };
}

/// What the combat hooks need to know about this frame: which local actors are the shared
/// enemies (the host's real ones, or the guest's copies) and who the two player bodies are.
#[derive(Debug, Clone, Default)]
pub(super) struct CombatView {
    pub(super) role: Option<EnemySyncRole>,
    pub(super) phase: u16,
    pub(super) enemies: HashMap<u64, EnemyId>,
    pub(super) local_player: u64,
    pub(super) puppet: u64,
    pub(super) aggro_targets: BTreeMap<EnemyId, AggroTarget>,
    pub(super) enemy_damage_bonus_percent: u32,
    pub(super) bullet_target: Option<PuppetBulletTarget>,
}

/// The guest's copy as the host's enemy bullets may test it: set only on the host while the
/// partner is in the same phase and not downed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PuppetBulletTarget {
    pub(super) handle: [u8; 8],
    pub(super) position: [f32; 3],
}

struct RemoteGuard(bool);

impl CombatView {
    pub(super) fn is_enemy(&self, actor: Option<u64>) -> Option<EnemyId> {
        actor.and_then(|actor| self.enemies.get(&actor).copied())
    }
}

impl Drop for RemoteGuard {
    fn drop(&mut self) {
        APPLYING_REMOTE.with(|flag| flag.set(self.0));
    }
}

pub(super) fn publish_view(view: CombatView) {
    *VIEW.lock().unwrap_or_else(PoisonError::into_inner) = view;
}

pub(super) fn add_shared_actors(actors: impl IntoIterator<Item = (u64, EnemyId)>) {
    VIEW.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .enemies
        .extend(actors);
}

pub(super) fn clear_view() {
    publish_view(CombatView::default());
}

pub(super) fn with_view<T>(read: impl FnOnce(&CombatView) -> T) -> T {
    read(&VIEW.lock().unwrap_or_else(PoisonError::into_inner))
}

pub(crate) fn applying_remote() -> bool {
    APPLYING_REMOTE.with(Cell::get)
}

pub(super) fn as_remote<T>(apply: impl FnOnce() -> T) -> T {
    let _restore = RemoteGuard(APPLYING_REMOTE.with(|flag| flag.replace(true)));
    apply()
}

pub(super) fn as_remote_with_kind<T>(
    kind: i32,
    source_class: Option<i32>,
    apply: impl FnOnce() -> T,
) -> T {
    PENDING_HIT_KIND.with(|pending| pending.set(Some((kind, source_class))));
    let result = as_remote(apply);
    PENDING_HIT_KIND.with(|pending| pending.set(None));
    result
}

pub(super) fn take_pending_hit_kind() -> Option<(i32, Option<i32>)> {
    PENDING_HIT_KIND.with(Cell::take)
}

/// As `as_remote`, and the record the game builds for the hit gets `reaction`'s reaction bits,
/// push and direction before the victim takes it.
pub(super) fn as_remote_with_reaction<T>(reaction: HitReaction, apply: impl FnOnce() -> T) -> T {
    PENDING_HIT_REACTION.with(|pending| pending.set(Some(reaction)));
    let result = as_remote(apply);
    PENDING_HIT_REACTION.with(|pending| pending.set(None));
    result
}

pub(super) fn take_pending_hit_reaction() -> Option<HitReaction> {
    PENDING_HIT_REACTION.with(Cell::take)
}
