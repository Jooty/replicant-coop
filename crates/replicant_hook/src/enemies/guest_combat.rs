use std::collections::{BTreeMap, HashMap, HashSet};

use replicant_game::GameMemory;
use replicant_game::enemies::{
    CopyStance, DAMAGE_STATE_NAME, EnemySighting, FIGHT_GUEST_ONLY_ENEMY_GRACE_MS,
    HOST_HIT_STAGGER_CHECK_WINDOW_MS, HitReaction, HostHas, HostListView, ListDiff, LocalKeep,
    MirroredEnemy, RemovalCause, SMALLEST_OWN_STEP_WORTH_LOGGING, StaggerVerdict,
    ghost_is_same_spawn, holds_copy, host_removal_destroys_copy, plan_ghosts,
    player_reaction_motion, read_player_state_name, sighting_text,
};
use replicant_game::npcs::villager_runs_own_event;
use replicant_game::offsets::{
    ACTOR_DRAWN_FLAG_OFFSET, ACTOR_SPAWN_SERIAL_OFFSET, ATTACK_FLAG_SKIP_DEFENCE,
    PLAYER_MESSAGE_DIRECT_HIT, PLAYER_MESSAGE_ENEMY_KILLED,
};
use replicant_game::puppet::actor_is_live;
use replicant_proto::{EnemyGoneReason, EnemyId, EnemyKilled, Message, MotionState};

use crate::boss_messages::is_boss;
use crate::diagnostics::log_line;
use crate::enemies::combat::{CombatState, GhostCopy};
use crate::enemies::combat_view::{CombatView, as_remote, as_remote_with_reaction, publish_view};
use crate::enemies::drop_hook::drop_at_actor;
use crate::enemies::hold_hook::{
    release_held, set_ghosts, set_held, set_stopped_dead, take_own_steps_cut,
};
use crate::enemies::sync::LocalView;
use crate::enemies::word_drop_hook::word_drop_at_actor;
use crate::enemies::{COPY_KILL_ATTEMPTS, EnemySyncRole, PENDING_KILL_MAX_AGE_MS, enemy_id_text};
use crate::memory::InProcessMemory;
use crate::paused_hits::paused_players_can_be_hit;
use crate::puppet::first_actor_address;
use crate::stay_together::player_in_menu;

const MENU_HIT_SKIP_LOG_INTERVAL_MS: u64 = 1000;

pub(super) fn receive(combat: &mut CombatState, now_ms: u64, message: &Message) {
    match message {
        Message::PlayerHit(hit) => combat.pending_player_hits.push(*hit),
        Message::EnemyKilled(killed) => {
            combat.host_ended_groups.insert(killed.id.group);
            combat.pending_kills.push((*killed, now_ms));
        }
        Message::EnemyCreation(creation) => {
            combat.creations.insert(creation.id, creation.clone());
        }
        Message::EnemyDrop(drop) => combat.pending_drops.push(*drop),
        Message::EnemyWordDrop(drop) => combat.pending_word_drops.push(*drop),
        Message::EnemyBodyGone(gone) => combat.pending_body_gone.push((gone.phase, gone.id)),
        Message::EnemyGone(gone) => {
            combat.host_ended_groups.insert(gone.id.group);
            match gone.reason {
                EnemyGoneReason::Killed => combat.pending_kills.push((
                    EnemyKilled {
                        phase: gone.phase,
                        id: gone.id,
                        by_guest: false,
                    },
                    now_ms,
                )),
                EnemyGoneReason::Removed => combat.pending_removals.push((gone.phase, gone.id)),
            }
        }
        _ => {}
    }
}

pub(super) fn step(
    combat: &mut CombatState,
    local: &LocalView<'_>,
    now_ms: u64,
    seen: &[(EnemyId, EnemySighting)],
    samples: &BTreeMap<EnemyId, MirroredEnemy>,
    authority: bool,
    host_view: &HostListView,
) {
    if combat.phase.is_some_and(|phase| phase != local.phase) {
        show_hidden_copies(combat, &mut InProcessMemory::new(local.build));
    }
    combat.enter_phase(local.phase);
    let live: HashSet<u64> = seen.iter().map(|(_, sighting)| sighting.actor).collect();
    combat.killed_copies.retain(|actor| live.contains(actor));
    combat.remember(
        seen.iter()
            .map(|(id, sighting)| (*id, sighting.actor, sighting.spawn_serial)),
    );
    let local_player = local.game.playable_actor().unwrap_or(0);
    let puppet = first_actor_address().unwrap_or(0);
    publish_view(CombatView {
        role: Some(EnemySyncRole::Guest),
        phase: local.phase,
        enemies: combat.actor_ids(),
        local_player,
        puppet,
        aggro_targets: BTreeMap::new(),
        enemy_damage_bonus_percent: 0,
        bullet_target: None,
    });
    let mut memory = InProcessMemory::new(local.build);
    let mut held = HashSet::new();
    for (id, sighting) in seen {
        // Only a copy of a host enemy waits for the host's death; one the host lacks lives here.
        if combat.killed_copies.contains(&sighting.actor) || !samples.contains_key(id) {
            continue;
        }
        if leaves_villager_to_own_logic(combat, local, *id, sighting.actor) {
            continue;
        }
        local
            .calls
            .set_survive_at_one_hp(&mut memory, sighting.actor, true);
        let host_motion = samples.get(id).and_then(|host| host.motion);
        let drift_units = samples.get(id).map_or(0.0, |host| {
            (sighting.position[0] - host.position[0]).hypot(sighting.position[2] - host.position[2])
        });
        if authority && copy_is_held(combat, local, *id, host_motion, drift_units) {
            held.insert(sighting.actor);
        }
    }
    if authority {
        held.extend(combat.ghosts.keys().copied());
    }
    combat.counts.held = held.len();
    set_held(held);
    hold_guest_only_copies(combat, local, &mut memory, now_ms, seen, samples, host_view);
    apply_player_hits(combat, local, now_ms, local_player);
    apply_kills(combat, local, &mut memory, now_ms, local_player, puppet);
    apply_drops(combat, local, local_player);
    apply_word_drops(combat, local, local_player);
    apply_removals(combat, local);
    watch_dying_bodies(combat, local, now_ms);
    set_stopped_dead(combat.dying_bodies.keys().copied().collect());
    log_own_steps_cut(combat, local.phase, seen, samples);
}

fn log_own_steps_cut(
    combat: &mut CombatState,
    phase: u16,
    seen: &[(EnemyId, EnemySighting)],
    samples: &BTreeMap<EnemyId, MirroredEnemy>,
) {
    for (actor, cut) in take_own_steps_cut() {
        if cut < SMALLEST_OWN_STEP_WORTH_LOGGING {
            continue;
        }
        let Some((id, _)) = seen.iter().find(|(_, sighting)| sighting.actor == actor) else {
            continue;
        };
        let Some(host) = samples.get(id) else {
            continue;
        };
        if !combat
            .stances
            .first_time(phase, *id, CopyStance::HeldAgainstOwnStep)
        {
            continue;
        }
        let host_motion = host
            .motion
            .map_or_else(|| "unknown".to_owned(), |motion| motion.index.to_string());
        log_line(&format!(
            "enemies: {} (host motion {host_motion}, host hp {})",
            CopyStance::HeldAgainstOwnStep.text(&enemy_id_text(*id), cut),
            host.hp
        ));
    }
}

pub(super) fn leaves_villager_to_own_logic(
    combat: &mut CombatState,
    local: &LocalView<'_>,
    id: EnemyId,
    actor: u64,
) -> bool {
    let runs_own_event = villager_runs_own_event(local.game.memory(), actor);
    let was_left_alone = combat.own_script_villagers.contains(&actor);
    if runs_own_event && !was_left_alone {
        combat.own_script_villagers.insert(actor);
        log_line(&format!(
            "combat: {} is a villager running a talk or script here, not held or moved by the enemy sync",
            enemy_id_text(id)
        ));
    } else if !runs_own_event && was_left_alone {
        combat.own_script_villagers.remove(&actor);
        log_line(&format!(
            "combat: {} is a villager with no talk or script here, the enemy sync holds it again",
            enemy_id_text(id)
        ));
    }
    runs_own_event
}

fn copy_is_held(
    combat: &mut CombatState,
    local: &LocalView<'_>,
    id: EnemyId,
    host_motion: Option<MotionState>,
    drift_units: f32,
) -> bool {
    let id_is_boss = is_boss(id);
    let held = holds_copy(host_motion.is_some());
    let stance = match (held, id_is_boss) {
        (true, true) => CopyStance::BossFollowsHostMotion,
        (false, _) => CopyStance::RunsOwnMotion,
        (true, false) => return held,
    };
    if combat.stances.first_time(local.phase, id, stance) {
        let motion = host_motion.map_or(0.0, |motion| f32::from(motion.index));
        log_line(&format!(
            "enemies: {}, {drift_units:.2} units from the host's body when this was first seen",
            stance.text(&enemy_id_text(id), motion)
        ));
    }
    held
}

fn hold_guest_only_copies(
    combat: &mut CombatState,
    local: &LocalView<'_>,
    memory: &mut InProcessMemory,
    now_ms: u64,
    seen: &[(EnemyId, EnemySighting)],
    samples: &BTreeMap<EnemyId, MirroredEnemy>,
    host_view: &HostListView,
) {
    let pending: HashSet<EnemyId> = combat
        .pending_kills
        .iter()
        .map(|(killed, _)| killed.id)
        .collect();
    let candidates: Vec<(EnemyId, u64)> = if host_view.is_known() {
        seen.iter()
            .filter(|(id, sighting)| {
                !samples.contains_key(id)
                    && host_view.host_has(*id) == HostHas::No
                    && !combat.ended_ids.contains(id)
                    && !pending.contains(id)
                    && !combat.ghosts.contains_key(&sighting.actor)
                    && !combat.killed_copies.contains(&sighting.actor)
                    && !combat.own_script_villagers.contains(&sighting.actor)
            })
            .map(|(id, sighting)| (*id, sighting.actor))
            .collect()
    } else {
        Vec::new()
    };
    let actors: Vec<u64> = candidates.iter().map(|(_, actor)| *actor).collect();
    let inside_fight_window = crate::fight_sync::covers_phase(local.phase);
    let expired = if inside_fight_window {
        combat
            .guest_only
            .expired_after(now_ms, &actors, FIGHT_GUEST_ONLY_ENEMY_GRACE_MS)
    } else {
        combat.guest_only.expired(now_ms, &actors)
    };
    for actor in expired {
        let Some((id, _)) = candidates.iter().find(|(_, other)| *other == actor) else {
            continue;
        };
        if inside_fight_window {
            remove_guest_only_fight_enemy(combat, local, samples.len(), actor, *id);
            continue;
        }
        let serial = memory
            .read_i32(actor + ACTOR_SPAWN_SERIAL_OFFSET)
            .unwrap_or_default();
        local.calls.set_survive_at_one_hp(memory, actor, true);
        combat.ghosts.insert(actor, GhostCopy { id: *id, serial });
        log_line(&format!(
            "combat: {} exists only here (the host's list of {} enemies is complete and lacks it); hidden and held, not removed, so your scripts do not move on",
            enemy_id_text(*id),
            samples.len()
        ));
        log_line(&format!(
            "enemies: held {}, cause: held because the host lacks it",
            describe(seen, actor, *id)
        ));
    }
    settle_ghosts(combat, local, memory, seen, samples);
    set_ghosts(combat.ghosts.keys().copied().collect());
}

fn remove_guest_only_fight_enemy(
    combat: &mut CombatState,
    local: &LocalView<'_>,
    host_count: usize,
    actor: u64,
    id: EnemyId,
) {
    // The host's fight stream runs the fight here, so no script of this game waits on it.
    release_held(actor);
    combat.killed_copies.insert(actor);
    combat
        .removal_causes
        .push((id, RemovalCause::GuestOnlyInFightWindow));
    local.calls.destroy(actor);
    log_line(&format!(
        "combat: {} exists only here inside a fight window (the host's list of {host_count} enemies lacks it); removed (no kill, no credit, no drop)",
        enemy_id_text(id)
    ));
}

fn settle_ghosts(
    combat: &mut CombatState,
    local: &LocalView<'_>,
    memory: &mut InProcessMemory,
    seen: &[(EnemyId, EnemySighting)],
    samples: &BTreeMap<EnemyId, MirroredEnemy>,
) {
    let current: HashMap<u64, EnemyId> = seen
        .iter()
        .map(|(id, sighting)| (sighting.actor, *id))
        .collect();
    let serials: HashMap<u64, i32> = seen
        .iter()
        .map(|(_, sighting)| (sighting.actor, sighting.spawn_serial))
        .collect();
    combat.ghosts.retain(|actor, ghost| {
        ghost_is_same_spawn(serials.get(actor).copied(), ghost.serial)
            && !combat.killed_copies.contains(actor)
    });
    let listed: Vec<(u64, EnemyId)> = combat
        .ghosts
        .keys()
        .filter_map(|actor| Some((*actor, *current.get(actor)?)))
        .collect();
    let host_live: HashSet<EnemyId> = samples.keys().copied().collect();
    let plan = plan_ghosts(&listed, &host_live, &combat.host_ended_groups);
    for actor in plan.adopt {
        let Some(ghost) = combat.ghosts.remove(&actor) else {
            continue;
        };
        show_copy(memory, actor, ghost.serial);
        log_line(&format!(
            "combat: {} now exists at the host; your hidden copy is shown and follows it",
            enemy_id_text(ghost.id)
        ));
        log_line(&format!(
            "enemies: shown {}, cause: the host spawned it",
            describe(seen, actor, ghost.id)
        ));
    }
    for actor in plan.release {
        let Some(ghost) = combat.ghosts.remove(&actor) else {
            continue;
        };
        release_held(actor);
        combat.killed_copies.insert(actor);
        combat
            .removal_causes
            .push((ghost.id, RemovalCause::SilentRemoval));
        local.calls.destroy(actor);
        log_line(&format!(
            "combat: {} exists only here and the host has finished group {}; hidden copy removed (no kill, no credit, no drop)",
            enemy_id_text(ghost.id),
            ghost.id.group
        ));
    }
    for actor in combat.ghosts.keys() {
        let _ = memory.write_u8(*actor + ACTOR_DRAWN_FLAG_OFFSET, 0);
    }
}

pub(super) fn show_copy(memory: &mut InProcessMemory, actor: u64, serial: i32) {
    let still_ours = actor_is_live(memory, actor).unwrap_or(false)
        && memory
            .read_i32(actor + ACTOR_SPAWN_SERIAL_OFFSET)
            .is_ok_and(|now| now == serial);
    if still_ours {
        let _ = memory.write_u8(actor + ACTOR_DRAWN_FLAG_OFFSET, 1);
    }
}

pub(super) fn show_hidden_copies(combat: &mut CombatState, memory: &mut InProcessMemory) {
    for (actor, ghost) in std::mem::take(&mut combat.ghosts) {
        show_copy(memory, actor, ghost.serial);
    }
    set_ghosts(HashSet::new());
}

fn apply_removals(combat: &mut CombatState, local: &LocalView<'_>) {
    let removals = std::mem::take(&mut combat.pending_removals);
    let memory = local.game.memory();
    for (phase, id) in removals {
        if phase != local.phase || !combat.ended_ids.insert(id) {
            continue;
        }
        let Some(copy) = combat.actor_of(memory, id) else {
            log_line(&format!(
                "combat: host removed {}, no copy here to remove",
                enemy_id_text(id)
            ));
            continue;
        };
        if combat.killed_copies.contains(&copy) {
            continue;
        }
        let max_hp = local.calls.hp_of(memory, copy).map(|(_, max_hp)| max_hp);
        if !host_removal_destroys_copy(max_hp, villager_runs_own_event(memory, copy)) {
            release_held(copy);
            log_line(&format!(
                "combat: host removed {}; your copy (max hp {}) is a story actor, left to your game's own script",
                enemy_id_text(id),
                max_hp.map_or_else(|| "unreadable".to_owned(), |max_hp| max_hp.to_string())
            ));
            continue;
        }
        release_held(copy);
        combat.killed_copies.insert(copy);
        combat.removal_causes.push((id, RemovalCause::HostRemoval));
        local.calls.destroy(copy);
        log_line(&format!(
            "combat: host removed {}; your copy removed too (reason: host removal)",
            enemy_id_text(id)
        ));
    }
}

fn apply_player_hits(
    combat: &mut CombatState,
    local: &LocalView<'_>,
    now_ms: u64,
    local_player: u64,
) {
    let hits = std::mem::take(&mut combat.pending_player_hits);
    let memory = local.game.memory();
    if local_player != 0
        && let Some(verdict) = combat
            .stagger_watch
            .observe(read_player_state_name(memory, local_player), now_ms)
    {
        log_line(&stagger_line(&verdict));
    }
    for hit in hits {
        if hit.phase != local.phase || local_player == 0 {
            log_line(&format!(
                "combat: host hit by {} on you not applied (phase {} here {}, player {local_player:#x})",
                enemy_id_text(hit.attacker),
                hit.phase,
                local.phase
            ));
            continue;
        }
        if !combat
            .player_hit_guard
            .counts(now_ms, hit.attacker, hit.hit_id)
        {
            continue;
        }
        if player_in_menu(local.build) && !paused_players_can_be_hit() {
            let quiet = combat
                .menu_hit_skip_logged_ms
                .is_some_and(|at| now_ms.saturating_sub(at) < MENU_HIT_SKIP_LOG_INTERVAL_MS);
            if !quiet {
                combat.menu_hit_skip_logged_ms = Some(now_ms);
                log_line("combat: hit on you skipped, you are in the menu");
            }
            continue;
        }
        let sender = combat
            .actor_of(memory, hit.attacker)
            .unwrap_or(local_player);
        let before = local.game.health().unwrap_or(-1);
        let state_before = read_player_state_name(memory, local_player);
        let reaction = HitReaction {
            flags: hit.flags,
            push: hit.push,
            direction: hit.direction,
        };
        let sent = as_remote_with_reaction(reaction, || {
            local.calls.send_actor_message(
                memory,
                local_player,
                PLAYER_MESSAGE_DIRECT_HIT,
                sender,
                u64::from(hit.hp_damage),
            )
        });
        let after = local.game.health().unwrap_or(-1);
        if sent {
            combat.counts.player_hits_applied += 1;
            combat.stagger_watch.hit_applied(
                &enemy_id_text(hit.attacker),
                reaction.kind(),
                hit.push,
                state_before == DAMAGE_STATE_NAME,
                now_ms,
            );
        }
        log_line(&format!(
            "combat: {} hit you at the host: hp {} sent, flags {:#x}, kind {}, reaction kind {} ({}) with push {:.2}, from {}, your body was in {state_before}, your hp {before} -> {after}",
            enemy_id_text(hit.attacker),
            hit.hp_damage,
            hit.flags,
            hit.attack_kind,
            reaction.kind(),
            player_reaction_motion(reaction.kind()),
            hit.push,
            if sender == local_player {
                "no local copy"
            } else {
                "its copy"
            }
        ));
    }
}

fn apply_kills(
    combat: &mut CombatState,
    local: &LocalView<'_>,
    memory: &mut InProcessMemory,
    now_ms: u64,
    local_player: u64,
    puppet: u64,
) {
    let pending = std::mem::take(&mut combat.pending_kills);
    for (killed, received_at) in pending {
        if killed.phase != local.phase || combat.ended_ids.contains(&killed.id) {
            continue;
        }
        let Some(copy) = combat.actor_of(memory, killed.id) else {
            if now_ms.saturating_sub(received_at) < PENDING_KILL_MAX_AGE_MS {
                combat.pending_kills.push((killed, received_at));
            } else {
                log_line(&format!(
                    "combat: host killed {}, no copy here to kill",
                    enemy_id_text(killed.id)
                ));
            }
            continue;
        };
        if combat.killed_copies.contains(&copy) {
            continue;
        }
        kill_copy(
            combat,
            local,
            memory,
            now_ms,
            killed,
            copy,
            local_player,
            puppet,
        );
    }
}

fn kill_copy(
    combat: &mut CombatState,
    local: &LocalView<'_>,
    memory: &mut InProcessMemory,
    now_ms: u64,
    killed: EnemyKilled,
    copy: u64,
    local_player: u64,
    puppet: u64,
) {
    let paid = local_player != 0
        && local.calls.send_actor_message(
            memory,
            local_player,
            PLAYER_MESSAGE_ENEMY_KILLED,
            copy,
            0,
        );
    combat.killed_copies.insert(copy);
    release_held(copy);
    local.calls.set_survive_at_one_hp(memory, copy, false);
    let attacker = if puppet == 0 { copy } else { puppet };
    let (hp_before, max_hp) = local.calls.hp_of(memory, copy).unwrap_or((1, 1));
    // Exactly lethal: a hit of 1.5 x max HP or more is an overkill (result bit 0x40000), which
    // the game kills differently from a normal death.
    let mut damage = hp_before.max(1);
    let mut hits = 0;
    let mut hp_after = hp_before;
    while hp_after > 0 && hits < COPY_KILL_ATTEMPTS {
        let dealt = damage.cast_unsigned();
        as_remote(|| {
            local
                .calls
                .direct_hit(attacker, copy, [dealt, 0, 0], ATTACK_FLAG_SKIP_DEFENCE);
        });
        hits += 1;
        hp_after = local.calls.hp_of(memory, copy).map_or(0, |(hp, _)| hp);
        damage = damage.saturating_mul(2);
    }
    combat.counts.kills_applied += 1;
    combat.ended_ids.insert(killed.id);
    combat.removal_causes.push((
        killed.id,
        if killed.by_guest {
            RemovalCause::KilledByGuest
        } else {
            RemovalCause::KilledByHost
        },
    ));
    let serial = memory
        .read_i32(copy + ACTOR_SPAWN_SERIAL_OFFSET)
        .unwrap_or_default();
    combat
        .dying_bodies
        .insert(copy, (killed.id, now_ms, serial));
    if combat
        .stances
        .first_time(local.phase, killed.id, CopyStance::DeadStopped)
    {
        log_line(&format!(
            "enemies: {}",
            CopyStance::DeadStopped.text(&enemy_id_text(killed.id), 0.0)
        ));
    }
    // A copy the hits could not kill is removed, so it never lingers alive after the host's death.
    let removed = hp_after > 0;
    if removed {
        local.calls.destroy(copy);
    }
    log_line(&format!(
        "combat: host killed {} (by {}); copy hp {hp_before}/{max_hp} -> {hp_after} in {hits} hit(s), {}, your kill credit {}",
        enemy_id_text(killed.id),
        if killed.by_guest { "you" } else { "the host" },
        if removed {
            "did not die so removed (reason: fallback)"
        } else {
            "dying"
        },
        if paid { "paid" } else { "not paid (no player)" }
    ));
}

fn watch_dying_bodies(combat: &mut CombatState, local: &LocalView<'_>, now_ms: u64) {
    for (phase, id) in std::mem::take(&mut combat.pending_body_gone) {
        if phase == local.phase {
            combat.host_gone_bodies.insert(id);
        }
    }
    let mut memory = InProcessMemory::new(local.build);
    let host_gone = &combat.host_gone_bodies;
    let stances = &mut combat.stances;
    combat.dying_bodies.retain(|actor, (id, killed_at, serial)| {
        let since_death = now_ms.saturating_sub(*killed_at);
        let still_there = actor_is_live(&memory, *actor).unwrap_or(false)
            && memory
                .read_i32(*actor + ACTOR_SPAWN_SERIAL_OFFSET)
                .is_ok_and(|now| now == *serial);
        if !still_there {
            log_line(&format!(
                "combat: your copy of {}'s body is gone {since_death} ms after its death (reason: the game removed it)",
                enemy_id_text(*id)
            ));
            return false;
        }
        if !host_gone.contains(id) {
            return true;
        }
        // Hidden, not destroyed: the copy's own death still ends it, so nothing frees it twice.
        let _ = memory.write_u8(*actor + ACTOR_DRAWN_FLAG_OFFSET, 0);
        if stances.first_time(local.phase, *id, CopyStance::HiddenWithHostBody) {
            log_line(&format!(
                "enemies: {}",
                CopyStance::HiddenWithHostBody.text(&enemy_id_text(*id), since_death as f32)
            ));
        }
        false
    });
}

fn apply_drops(combat: &mut CombatState, local: &LocalView<'_>, local_player: u64) {
    let drops = std::mem::take(&mut combat.pending_drops);
    let memory = local.game.memory();
    for drop in drops {
        if drop.phase != local.phase {
            continue;
        }
        let copy = combat.actor_of(memory, drop.id);
        let at = copy.unwrap_or(local_player);
        let placed = drop_at_actor(at, drop.item_id, drop.count);
        if placed {
            combat.counts.drops_applied += 1;
        }
        log_line(&format!(
            "combat: host's {} dropped item {} x{}: {} at {}",
            enemy_id_text(drop.id),
            drop.item_id,
            drop.count,
            if placed {
                "dropped here"
            } else {
                "not dropped"
            },
            if copy.is_some() {
                "your copy"
            } else {
                "your feet"
            }
        ));
    }
}

fn apply_word_drops(combat: &mut CombatState, local: &LocalView<'_>, local_player: u64) {
    let drops = std::mem::take(&mut combat.pending_word_drops);
    let memory = local.game.memory();
    for drop in drops {
        if drop.phase != local.phase {
            continue;
        }
        let copy = combat.actor_of(memory, drop.id);
        let at = copy.unwrap_or(local_player);
        let placed = word_drop_at_actor(at, drop.word_id);
        log_line(&format!(
            "combat: host's {} dropped word {}: {} at {}",
            enemy_id_text(drop.id),
            drop.word_id,
            if placed {
                "dropped here"
            } else {
                "not dropped"
            },
            if copy.is_some() {
                "your copy"
            } else {
                "your feet"
            }
        ));
    }
}

pub(super) fn list_diff(
    combat: &CombatState,
    seen: &[(EnemyId, EnemySighting)],
    samples: &BTreeMap<EnemyId, MirroredEnemy>,
    host_view: &HostListView,
) -> ListDiff {
    let here: HashSet<EnemyId> = seen.iter().map(|(id, _)| *id).collect();
    let pending: HashSet<EnemyId> = combat
        .pending_kills
        .iter()
        .map(|(killed, _)| killed.id)
        .collect();
    ListDiff {
        only_host: samples
            .keys()
            .filter(|id| !here.contains(id))
            .copied()
            .collect(),
        only_local: seen
            .iter()
            .filter(|(id, _)| !samples.contains_key(id))
            .map(|(id, sighting)| {
                let keep = if combat.ghosts.contains_key(&sighting.actor) {
                    LocalKeep::HeldHidden
                } else if pending.contains(id) {
                    LocalKeep::WaitingHostKill
                } else if combat.ended_ids.contains(id) {
                    LocalKeep::HostEnded
                } else if !host_view.is_known() {
                    LocalKeep::HostListIncomplete
                } else if host_view.host_has(*id) == HostHas::Yes {
                    LocalKeep::OutsideHostRound
                } else {
                    LocalKeep::WaitingGrace
                };
                (*id, keep)
            })
            .collect(),
    }
}

fn describe(seen: &[(EnemyId, EnemySighting)], actor: u64, id: EnemyId) -> String {
    seen.iter()
        .find(|(_, sighting)| sighting.actor == actor)
        .map_or_else(
            || enemy_id_text(id),
            |(id, sighting)| sighting_text(*id, sighting),
        )
}

fn stagger_line(verdict: &StaggerVerdict) -> String {
    match verdict {
        StaggerVerdict::Entered { check, after_ms } => format!(
            "combat: stagger: your body entered {DAMAGE_STATE_NAME} {after_ms} ms after {}'s hit at the host (reaction kind {}, {}, push {:.2})",
            check.attacker,
            check.kind,
            player_reaction_motion(check.kind),
            check.push
        ),
        StaggerVerdict::NotEntered { check, last_state } => format!(
            "combat: stagger: your body did not enter {DAMAGE_STATE_NAME} within {HOST_HIT_STAGGER_CHECK_WINDOW_MS} ms of {}'s hit at the host (reaction kind {}, {}, push {:.2}), it is in {last_state}: the action it was in did not take that reaction",
            check.attacker,
            check.kind,
            player_reaction_motion(check.kind),
            check.push
        ),
    }
}
