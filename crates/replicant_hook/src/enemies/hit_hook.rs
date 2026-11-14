use core::ffi::c_void;
use core::ptr;
use std::cell::Cell;
use std::collections::HashMap;
use std::panic;
use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use replicant_game::GameMemory;
use replicant_game::boss_messages::held_hp_to_restore;
use replicant_game::effects::{projectile_kind, projectile_owner, read_actor_position};
use replicant_game::enemies::{
    AggroTarget, AttackRecord, EnemyHitLogLimiter, GUEST_HIT_REPEAT_WINDOW_MS, HitReaction,
    OwnHitInputs, OwnHitLimiter, OwnHitRecord, PlayerHitGuard, companion_hit_counts,
    credited_enemy, own_hit_text, pack_hit_kind, restored_time_scale, scaled_hp_damage,
    spec_class_of,
};
use replicant_game::enemy_bullets::bullet_owner;
use replicant_game::offsets::BODY_MOTION_LAYER;
use replicant_game::offsets::{
    ACTOR_TIME_SCALE_OFFSET, ATTACK_FLAG_MAGIC, ATTACK_RECORD_ATTACK_KIND_OFFSET,
    ATTACK_RECORD_FLAGS_OFFSET, ATTACK_RECORD_HIT_DIRECTION_OFFSET, ATTACK_RECORD_HP_DAMAGE_OFFSET,
    ATTACK_RECORD_PUSH_STRENGTH_OFFSET, ATTACK_RECORD_SOURCE_CLASS_OFFSET,
    RECEIVE_ATTACK_QUERY_FUNCTION_RVA,
};
use replicant_game::puppet::{body_info, motion_name, read_layer_motion};
use replicant_proto::{EnemyHit, EnemyId, MAX_HIT_DAMAGE, Message, PlayerHit};

use crate::bombs::is_own_bomb_hit;
use crate::boss_messages::is_boss;
use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::effects::{begin_own_hit, end_own_hit};
use crate::enemies::EnemySyncRole;
use crate::enemies::boss_hit_log::log_host_boss_hit;
use crate::enemies::combat_view::{
    CombatView, applying_remote, take_pending_hit_kind, take_pending_hit_reaction, with_view,
};
use crate::enemies::counters::{HookCount, count};
use crate::enemies::enemy_id_text;
use crate::enemies::game_calls::EnemyCalls;
use crate::enemies::hold_hook::is_ghost;
use crate::game_hooks::hook_address;
use crate::memory::InProcessMemory;
use crate::net;
use crate::player_body::real_body;
use crate::story_relay::shared_story_events;

static HOOKED_BUILD: OnceLock<VerifiedBuild> = OnceLock::new();
static ORIGINAL_RECEIVE: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static LAST_LOGGED_MAGIC_HIT: AtomicU32 = AtomicU32::new(0);
static BODY_SWINGS: LazyLock<Mutex<PlayerHitGuard>> = LazyLock::new(Mutex::default);
static BODY_SWING_CLOCK: LazyLock<Instant> = LazyLock::new(Instant::now);
static ENEMY_HIT_LOGS: LazyLock<Mutex<EnemyHitLogLimiter>> = LazyLock::new(Mutex::default);
static OWN_HITS: LazyLock<Mutex<OwnHitLimiter>> = LazyLock::new(Mutex::default);
static REPORTED_SWINGS: LazyLock<Mutex<HashMap<(EnemyId, u32), Instant>>> =
    LazyLock::new(Mutex::default);

thread_local! {
    static OWN_HIT_ON_COMPANION: Cell<Option<OwnHitOnCompanion>> = const { Cell::new(None) };
}

type ReceiveAttackQuery =
    unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void, i32, u8) -> u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HitRoute {
    Run,
    Drop,
}

#[derive(Debug, Clone, Copy)]
struct CreditedAttacker {
    actor: u64,
    enemy: EnemyId,
    by_bullet: bool,
}

#[derive(Debug, Clone, Copy)]
struct OwnHitOnCompanion {
    victim: u64,
    phase: u16,
    id: EnemyId,
    attack: AttackRecord,
    hp_before: Option<i32>,
}

pub(super) fn install(build: VerifiedBuild) {
    if HOOKED_BUILD.set(build).is_err() {
        return;
    }
    #[expect(
        unsafe_code,
        reason = "hooks CActor::ReceiveAttackQuery by its address"
    )]
    // SAFETY: the RVA starts the base vslot 0x5D8 body every actor class uses (victim, attack
    // record, result, part index, force flag) in the verified build; the replacement has the
    // same five arguments and passes them on unchanged.
    unsafe {
        hook_address(
            "CActor::ReceiveAttackQuery",
            (build.module_base() + u64::from(RECEIVE_ATTACK_QUERY_FUNCTION_RVA)) as usize,
            receive_attack_query_detour as *const () as *mut c_void,
            &ORIGINAL_RECEIVE,
        );
    }
}

#[expect(
    unsafe_code,
    reason = "calls the original ReceiveAttackQuery through the trampoline MinHook built"
)]
extern "system" fn receive_attack_query_detour(
    victim: *mut c_void,
    record: *mut c_void,
    result: *mut c_void,
    part: i32,
    force: u8,
) -> u64 {
    if let Some(build) = HOOKED_BUILD.get() {
        let _ = panic::catch_unwind(|| {
            crate::kaine::note_hit_for_companion_off(build, victim as u64, record as u64);
        });
    }
    let route = if applying_remote() {
        HitRoute::Run
    } else {
        panic::catch_unwind(|| route_hit(victim as u64, record as u64)).unwrap_or(HitRoute::Run)
    };
    if route == HitRoute::Drop {
        end_own_hit();
        return 0;
    }
    let original = ORIGINAL_RECEIVE.load(Ordering::Acquire);
    if original.is_null() {
        end_own_hit();
        OWN_HIT_ON_COMPANION.with(Cell::take);
        return 0;
    }
    if applying_remote() {
        let _ = panic::catch_unwind(|| write_pending_hit_kind(record as u64));
        let _ = panic::catch_unwind(|| write_pending_hit_reaction(record as u64));
    }
    let scale_before = panic::catch_unwind(|| struck_copy_time_scale(victim as u64))
        .ok()
        .flatten();
    let hp_before = if applying_remote() {
        None
    } else {
        panic::catch_unwind(|| struck_copy_hp(victim as u64))
            .ok()
            .flatten()
    };
    let host_boss_before = if applying_remote() {
        None
    } else {
        panic::catch_unwind(|| host_boss_hp(victim as u64, record as u64))
            .ok()
            .flatten()
    };
    // SAFETY: the trampoline continues into the original body, which has this exact signature.
    let outcome = unsafe {
        core::mem::transmute::<*mut c_void, ReceiveAttackQuery>(original)(
            victim, record, result, part, force,
        )
    };
    end_own_hit();
    let _ = panic::catch_unwind(|| report_own_hit_the_companion_took(victim as u64));
    if let Some((id, before, attack)) = host_boss_before {
        let _ = panic::catch_unwind(|| log_host_boss_damage(victim as u64, id, before, attack));
    }
    if let Some(before) = scale_before {
        let _ = panic::catch_unwind(|| restore_time_scale(victim as u64, before));
    }
    if let Some(before) = hp_before {
        let _ = panic::catch_unwind(|| restore_hp(victim as u64, before));
    }
    outcome
}

fn write_pending_hit_kind(record: u64) {
    let Some((kind, source_class)) = take_pending_hit_kind() else {
        return;
    };
    let Some(build) = HOOKED_BUILD.get() else {
        return;
    };
    let mut memory = InProcessMemory::new(build);
    let _ = memory.write_i32(record + ATTACK_RECORD_ATTACK_KIND_OFFSET, kind);
    if let Some(class) = source_class {
        let _ = memory.write_i32(record + ATTACK_RECORD_SOURCE_CLASS_OFFSET, class);
    }
}

fn write_pending_hit_reaction(record: u64) {
    let Some(reaction) = take_pending_hit_reaction() else {
        return;
    };
    let Some(build) = HOOKED_BUILD.get() else {
        return;
    };
    let mut memory = InProcessMemory::new(build);
    let Ok(flags) = memory.read_i32(record + ATTACK_RECORD_FLAGS_OFFSET) else {
        return;
    };
    let flags = reaction.flags_for_direct_hit(flags.cast_unsigned());
    let _ = memory.write_i32(record + ATTACK_RECORD_FLAGS_OFFSET, flags.cast_signed());
    let _ = memory.write_f32(record + ATTACK_RECORD_PUSH_STRENGTH_OFFSET, reaction.push);
    for (axis, value) in (0_u64..).zip(reaction.direction) {
        let _ = memory.write_f32(
            record + ATTACK_RECORD_HIT_DIRECTION_OFFSET + axis * 4,
            value,
        );
    }
}

fn host_boss_hp(victim: u64, record: u64) -> Option<(EnemyId, i32, AttackRecord)> {
    let build = HOOKED_BUILD.get()?;
    let id = with_view(|view| {
        if view.role == Some(EnemySyncRole::Host) {
            view.enemies.get(&victim).copied()
        } else {
            None
        }
    })?;
    if !is_boss(id) {
        return None;
    }
    let memory = InProcessMemory::new(build);
    let attack = AttackRecord::read(&memory, record)?;
    EnemyCalls::new(build)
        .hp_of(&memory, victim)
        .map(|(hp, _)| (id, hp, attack))
}

fn log_host_boss_damage(victim: u64, id: EnemyId, before: i32, attack: AttackRecord) {
    let Some(build) = HOOKED_BUILD.get() else {
        return;
    };
    let memory = InProcessMemory::new(build);
    let after = EnemyCalls::new(build)
        .hp_of(&memory, victim)
        .map_or(before, |(hp, _)| hp);
    log_host_boss_hit(build, victim, before, after, &attack);
    if after == before {
        return;
    }
    log_line(&format!(
        "boss damage: from host, {} amount {}, hp left {after}",
        enemy_id_text(id),
        before - after
    ));
}

fn struck_copy_hp(victim: u64) -> Option<i32> {
    let build = HOOKED_BUILD.get()?;
    if !shared_story_events() {
        return None;
    }
    let copy_id = with_view(|view| {
        if view.role == Some(EnemySyncRole::Guest) {
            view.enemies.get(&victim).copied()
        } else {
            None
        }
    })?;
    if !is_boss(copy_id) {
        return None;
    }
    let memory = InProcessMemory::new(build);
    EnemyCalls::new(build)
        .hp_of(&memory, victim)
        .map(|(hp, _)| hp)
}

fn restore_hp(victim: u64, before: i32) {
    let Some(build) = HOOKED_BUILD.get() else {
        return;
    };
    let memory = InProcessMemory::new(build);
    let calls = EnemyCalls::new(build);
    let Some((after, _)) = calls.hp_of(&memory, victim) else {
        return;
    };
    let Some(target) = held_hp_to_restore(before, after) else {
        return;
    };
    // The host's HP snapshots own a held copy's HP; a local hit must not trip a life threshold early.
    if calls.set_hp(&memory, victim, target) {
        log_line(&format!(
            "combat: a hit lowered the hp of your copy at {victim:#x} from {before} to {after}; restored to {target}"
        ));
    }
}

fn struck_copy_time_scale(victim: u64) -> Option<f32> {
    let build = HOOKED_BUILD.get()?;
    let is_guest_copy = with_view(|view| {
        view.role == Some(EnemySyncRole::Guest) && view.enemies.contains_key(&victim)
    });
    if !is_guest_copy {
        return None;
    }
    InProcessMemory::new(build)
        .read_f32(victim + ACTOR_TIME_SCALE_OFFSET)
        .ok()
}

fn restore_time_scale(victim: u64, before: f32) {
    let Some(build) = HOOKED_BUILD.get() else {
        return;
    };
    let mut memory = InProcessMemory::new(build);
    let Ok(after) = memory.read_f32(victim + ACTOR_TIME_SCALE_OFFSET) else {
        return;
    };
    let Some(target) = restored_time_scale(before, after) else {
        return;
    };
    // A held copy never runs the state step that ends the hit's slow-down, so its motion would stay frozen.
    if memory
        .write_f32(victim + ACTOR_TIME_SCALE_OFFSET, target)
        .is_ok()
    {
        log_line(&format!(
            "combat: a hit changed the time scale of your copy at {victim:#x} from {before:.2} to {after:.2}; restored to {target:.2}"
        ));
    }
}

fn route_hit(victim: u64, record: u64) -> HitRoute {
    let Some(build) = HOOKED_BUILD.get() else {
        return HitRoute::Run;
    };
    with_view(|view| match view.role {
        None => HitRoute::Run,
        Some(EnemySyncRole::Guest) => guest_route(build, view, victim, record),
        Some(EnemySyncRole::Host) => {
            host_watch(build, view, victim, record);
            HitRoute::Run
        }
    })
}

fn guest_route(build: &VerifiedBuild, view: &CombatView, victim: u64, record: u64) -> HitRoute {
    if is_ghost(victim) {
        return HitRoute::Drop;
    }
    let victim_copy = view.enemies.get(&victim).copied();
    if victim_copy.is_none() && victim != view.local_player {
        return HitRoute::Run;
    }
    let memory = InProcessMemory::new(build);
    let Some(attack) = AttackRecord::read(&memory, record) else {
        return HitRoute::Run;
    };
    let sources = attack.sources(&memory);
    if let Some(id) = victim_copy {
        let bullet_of_local = sources[0].and_then(|attacker| projectile_owner(&memory, attacker));
        log_magic_hit(
            &memory,
            &attack,
            sources,
            bullet_of_local,
            (view.local_player, id),
        );
        let fired_by_local = sources.contains(&Some(view.local_player))
            || bullet_of_local == Some(view.local_player)
            || is_own_bomb_hit(attack.hit_id);
        if view.local_player != 0 && fired_by_local {
            begin_own_hit();
            log_own_hit(build, view, &attack);
            let attack =
                crate::kaine::hit_at_first_body_power(build, view.local_player, record, attack);
            if id.is_companion() {
                // Sent after the game's own answer, and only when the hit lowered its life here.
                OWN_HIT_ON_COMPANION.with(|pending| {
                    pending.set(Some(OwnHitOnCompanion {
                        victim,
                        phase: view.phase,
                        id,
                        attack,
                        hp_before: actor_hp(build, victim),
                    }));
                });
            } else {
                report_hit(view.phase, id, &attack);
            }
        }
        return HitRoute::Run;
    }
    // The host decides every enemy hit on this player; a held copy's own swing never lands.
    if let Some(attacker) = sources.iter().find_map(|source| view.is_enemy(*source)) {
        count(HookCount::LocalEnemyHitBlocked);
        log_line(&format!(
            "combat: blocked {}'s local hit on you (hp {}, kind {}); the host sends enemy hits",
            enemy_id_text(attacker),
            attack.hp_damage,
            attack.attack_kind
        ));
        return HitRoute::Drop;
    }
    HitRoute::Run
}

fn log_host_own_hit(build: &VerifiedBuild, view: &CombatView, record: u64) {
    let memory = InProcessMemory::new(build);
    let Some(attack) = AttackRecord::read(&memory, record) else {
        return;
    };
    let sources = attack.sources(&memory);
    let bullet_of_local = sources[0].and_then(|attacker| projectile_owner(&memory, attacker));
    if sources.contains(&Some(view.local_player)) || bullet_of_local == Some(view.local_player) {
        begin_own_hit();
        log_own_hit(build, view, &attack);
    }
}

fn log_own_hit(build: &VerifiedBuild, view: &CombatView, attack: &AttackRecord) {
    if !OWN_HITS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .allow(view.phase)
    {
        return;
    }
    let memory = InProcessMemory::new(build);
    let inputs = OwnHitInputs::read(&memory, view.local_player);
    let motion = read_layer_motion(&memory, view.local_player, BODY_MOTION_LAYER)
        .ok()
        .flatten()
        .and_then(|layer| {
            motion_name(&memory, view.local_player, layer.index)
                .ok()
                .flatten()
        });
    let body =
        real_body().map(|real| (body_info(real.body).own_seed_name, spec_class_of(real.body)));
    let record = OwnHitRecord {
        hp_damage: attack.hp_damage,
        second_gauge: attack.second_gauge,
        third_gauge: attack.third_gauge,
    };
    log_line(&own_hit_text(&inputs, record, motion.as_deref(), body));
}

fn log_magic_hit(
    memory: &InProcessMemory,
    attack: &AttackRecord,
    sources: [Option<u64>; 2],
    bullet_of_local: Option<u64>,
    (local_player, enemy): (u64, EnemyId),
) {
    if attack.flags & ATTACK_FLAG_MAGIC == 0
        || LAST_LOGGED_MAGIC_HIT.swap(attack.hit_id, Ordering::Relaxed) == attack.hit_id
    {
        return;
    }
    log_line(&format!(
        "combat: magic hit {}: source class {}, attacker found {}, owner is you {}, bullet owner is you {}",
        attack.hit_id,
        attack.source_class,
        sources[0].is_some(),
        sources[1] == Some(local_player),
        bullet_of_local == Some(local_player)
    ));
    let kind = sources[0].and_then(|attacker| projectile_kind(memory, attacker));
    if bullet_of_local == Some(local_player)
        && let Some(kind) = kind
    {
        log_line(&format!(
            "combat: magic hit by your {kind} on {}, hit {}",
            enemy_id_text(enemy),
            attack.hit_id
        ));
    }
}

fn log_enemy_hit_on_player(build: &VerifiedBuild, view: &CombatView, victim: u64, record: u64) {
    let victim_text = if view.local_player != 0 && victim == view.local_player {
        "host body"
    } else if view.puppet != 0 && victim == view.puppet {
        "guest puppet"
    } else {
        return;
    };
    let memory = InProcessMemory::new(build);
    let Some(attack) = AttackRecord::read(&memory, record) else {
        return;
    };
    let Some(credit) = credited_attacker(&memory, view, &attack) else {
        return;
    };
    let (attacker_actor, attacker) = (credit.actor, credit.enemy);
    let now_ms = u64::try_from(BODY_SWING_CLOCK.elapsed().as_millis()).unwrap_or(u64::MAX);
    if !ENEMY_HIT_LOGS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .allow(now_ms, attacker)
    {
        return;
    }
    let target = view
        .aggro_targets
        .get(&attacker)
        .map_or("none pushed", |target| match target {
            AggroTarget::Host => "host",
            AggroTarget::Guest => "guest",
        });
    let at = |actor: u64| {
        read_actor_position(&memory, actor).map_or_else(
            |_| "?".to_owned(),
            |p| format!("({:.1},{:.1},{:.1})", p[0], p[1], p[2]),
        )
    };
    log_line(&format!(
        "combat: hit by {} on {victim_text} (hp {}, {}): target {target}, attacker {}, host {}, puppet {}",
        enemy_id_text(attacker),
        attack.hp_damage,
        if credit.by_bullet {
            "by its bullet"
        } else {
            "by its body"
        },
        at(attacker_actor),
        if view.local_player != 0 {
            at(view.local_player)
        } else {
            "?".to_owned()
        },
        if view.puppet != 0 {
            at(view.puppet)
        } else {
            "?".to_owned()
        },
    ));
}

fn host_watch(build: &VerifiedBuild, view: &CombatView, victim: u64, record: u64) {
    log_enemy_hit_on_player(build, view, victim, record);
    if view.local_player != 0 && victim == view.local_player {
        scale_enemy_hit_on_host(build, view, record);
        return;
    }
    if view.local_player != 0 && view.enemies.contains_key(&victim) {
        log_host_own_hit(build, view, record);
        return;
    }
    if view.puppet == 0 || victim != view.puppet {
        return;
    }
    let memory = InProcessMemory::new(build);
    let Some(attack) = AttackRecord::read(&memory, record) else {
        return;
    };
    let Some(credit) = credited_attacker(&memory, view, &attack) else {
        return;
    };
    let attacker = credit.enemy;
    let Some(net) = net::handle() else {
        return;
    };
    let now_ms = u64::try_from(BODY_SWING_CLOCK.elapsed().as_millis()).unwrap_or(u64::MAX);
    let counts = BODY_SWINGS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .counts(now_ms, attacker, attack.hit_id);
    if !counts {
        return;
    }
    let reaction = HitReaction::read(&memory, record).unwrap_or(HitReaction {
        flags: attack.flags,
        push: 0.0,
        direction: [0.0; 3],
    });
    let hit = PlayerHit {
        phase: view.phase,
        attacker,
        hp_damage: damage_on_wire(scaled_hp_damage(
            attack.hp_damage,
            view.enemy_damage_bonus_percent,
        )),
        flags: attack.flags,
        attack_kind: attack.attack_kind.cast_unsigned(),
        hit_id: attack.hit_id,
        push: reaction.push,
        direction: reaction.direction,
    };
    net.send_relayed(Message::PlayerHit(hit));
    count(HookCount::PlayerHitSent);
    log_line(&format!(
        "combat: {} hit the guest's body{}: hp {} (was {}, +{}%), flags {:#x}, kind {}, hit id {}, reaction kind {}, push {:.2}, direction ({:.2}, {:.2}, {:.2}) (sent)",
        enemy_id_text(attacker),
        if credit.by_bullet {
            " with a bullet"
        } else {
            ""
        },
        hit.hp_damage,
        attack.hp_damage,
        view.enemy_damage_bonus_percent,
        hit.flags,
        hit.attack_kind,
        attack.hit_id,
        reaction.kind(),
        hit.push,
        hit.direction[0],
        hit.direction[1],
        hit.direction[2]
    ));
}

fn scale_enemy_hit_on_host(build: &VerifiedBuild, view: &CombatView, record: u64) {
    if view.enemy_damage_bonus_percent == 0 {
        return;
    }
    let mut memory = InProcessMemory::new(build);
    let Some(attack) = AttackRecord::read(&memory, record) else {
        return;
    };
    let Some(attacker) = credited_attacker(&memory, view, &attack).map(|credit| credit.enemy)
    else {
        return;
    };
    let scaled = scaled_hp_damage(attack.hp_damage, view.enemy_damage_bonus_percent);
    if scaled == attack.hp_damage
        || memory
            .write_i32(record + ATTACK_RECORD_HP_DAMAGE_OFFSET, scaled)
            .is_err()
    {
        return;
    }
    log_line(&format!(
        "difficulty: {} hit you for hp {scaled} (was {}, +{}%), hit id {}",
        enemy_id_text(attacker),
        attack.hp_damage,
        view.enemy_damage_bonus_percent,
        attack.hit_id
    ));
}

fn credited_attacker(
    memory: &InProcessMemory,
    view: &CombatView,
    attack: &AttackRecord,
) -> Option<CreditedAttacker> {
    let sources = attack.sources(memory);
    let owner = sources[0].and_then(|attacker| bullet_owner(memory, attacker));
    let (actor, enemy) = credited_enemy(sources, owner, |actor| view.enemies.get(&actor).copied())?;
    Some(CreditedAttacker {
        actor,
        enemy,
        by_bullet: !sources.contains(&Some(actor)),
    })
}

fn actor_hp(build: &VerifiedBuild, actor: u64) -> Option<i32> {
    EnemyCalls::new(build)
        .hp_of(&InProcessMemory::new(build), actor)
        .map(|(hp, _)| hp)
}

fn report_own_hit_the_companion_took(victim: u64) {
    let Some(hit) = OWN_HIT_ON_COMPANION.with(Cell::take) else {
        return;
    };
    let Some(build) = HOOKED_BUILD.get() else {
        return;
    };
    if hit.victim == victim && companion_hit_counts(hit.hp_before, actor_hp(build, victim)) {
        report_hit(hit.phase, hit.id, &hit.attack);
    }
}

fn first_report_of_swing(id: EnemyId, hit_id: u32) -> bool {
    // The game hands the same swing to a held copy every frame it overlaps; hit id 0 is no swing.
    if hit_id == 0 {
        return true;
    }
    let now = Instant::now();
    let window = Duration::from_millis(GUEST_HIT_REPEAT_WINDOW_MS);
    let mut reported = REPORTED_SWINGS
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    reported.retain(|_, at| now.saturating_duration_since(*at) < window);
    reported.insert((id, hit_id), now).is_none()
}

fn report_hit(phase: u16, id: EnemyId, attack: &AttackRecord) {
    if !first_report_of_swing(id, attack.hit_id) {
        return;
    }
    let Some(net) = net::handle() else {
        return;
    };
    let hit = EnemyHit {
        phase,
        id,
        hit_id: attack.hit_id,
        hp_damage: damage_on_wire(attack.hp_damage),
        second_gauge: damage_on_wire(attack.second_gauge),
        third_gauge: damage_on_wire(attack.third_gauge),
        flags: attack.flags,
        attack_kind: pack_hit_kind(attack.attack_kind, attack.source_class),
    };
    net.send_relayed(Message::EnemyHit(hit));
    count(HookCount::HitSent);
    log_line(&format!(
        "combat: you hit {}: hp {}, gauges {}/{}, flags {:#x}, kind {}, source {}, hit id {} (sent)",
        enemy_id_text(id),
        hit.hp_damage,
        hit.second_gauge,
        hit.third_gauge,
        hit.flags,
        attack.attack_kind,
        attack.source_class,
        hit.hit_id
    ));
}

fn damage_on_wire(damage: i32) -> u32 {
    damage
        .clamp(0, MAX_HIT_DAMAGE.cast_signed())
        .cast_unsigned()
}
