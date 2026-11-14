use std::collections::HashMap;

use replicant_game::GameMemory;
use replicant_game::downed::DownedState;
use replicant_game::effects::read_actor_position;
use replicant_game::enemies::{
    AGGRO_PUSH_WEIGHT, AggroPlayers, AggroTarget, EXTRA_PLAYERS_WITH_ONE_GUEST, EnemySighting,
    GuestHitDamage, HitVerdict, RosterUpdate, ScaledEnemyKey, damage_dealt_bonus_percent,
    damage_taken_factor, guest_hit_damage, unpack_hit_kind, victim_refuses_hit,
};
use replicant_game::kaine::GUEST_HIT_SCALE_IN_A_STAND_IN_BEAT;
use replicant_game::offsets::{
    ACTOR_KIND_ENEMY_TARGET_STAND_IN_BIT, ACTOR_KIND_MASK_OFFSET, ACTOR_KIND_SHORT_TICK_BIT,
    ACTOR_SELF_HANDLE_OFFSET, ACTOR_SPAWN_SERIAL_OFFSET, GUEST_HIT_FLAGS_KEPT,
    PLAYER_MESSAGE_ENEMY_KILLED,
};
use replicant_game::puppet::actor_is_live;
use replicant_proto::{
    EnemyBodyGone, EnemyGoneReason, EnemyHit, EnemyId, EnemyKilled, Message, PartnerActivity,
    SessionOptions,
};

use crate::boss_messages::is_boss;
use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::downed::downed_status;
use crate::enemies::EnemySyncRole;
use crate::enemies::boss_hit_log::log_guest_boss_hit;
use crate::enemies::combat::{CombatState, PuppetKind, read_kind};
use crate::enemies::combat_view::{
    CombatView, PuppetBulletTarget, as_remote_with_kind, publish_view,
};
use crate::enemies::enemy_id_text;
use crate::enemies::game_calls::EnemyCalls;
use crate::enemies::gauge_opening_replay::remember_guest_boss_hit;
use crate::enemies::sync::LocalView;
use crate::memory::InProcessMemory;
use crate::net::{NetThread, RemotePlayer};
use crate::puppet::first_actor_address;
use crate::stay_together::player_in_menu;

pub(super) fn apply_guest_hits(
    combat: &mut CombatState,
    local: &LocalView<'_>,
    now_ms: u64,
    hits: &[EnemyHit],
) {
    for hit in hits {
        let verdict = combat.guard.judge(now_ms, local.phase, hit);
        if verdict != HitVerdict::Apply {
            combat.counts.hits_refused += 1;
            let key = (hit.id, hit.hit_id);
            if verdict == HitVerdict::Repeat && combat.last_repeat_logged == Some(key) {
                continue;
            }
            if verdict == HitVerdict::Repeat {
                combat.last_repeat_logged = Some(key);
            }
            log_line(&format!(
                "combat: guest hit on {} hit id {} not applied ({verdict:?})",
                enemy_id_text(hit.id),
                hit.hit_id
            ));
            continue;
        }
        apply_guest_hit(combat, local, hit);
    }
}

fn apply_guest_hit(combat: &mut CombatState, local: &LocalView<'_>, hit: &EnemyHit) {
    let memory = local.game.memory();
    let Some(enemy) = combat.actor_of(memory, hit.id) else {
        combat.counts.hits_refused += 1;
        log_line(&format!(
            "combat: guest hit on {} not applied (no such enemy here now)",
            enemy_id_text(hit.id)
        ));
        return;
    };
    let Some(puppet) = first_actor_address().filter(|actor| *actor != 0) else {
        combat.counts.hits_refused += 1;
        log_line(&format!(
            "combat: guest hit on {} not applied (the guest has no body here)",
            enemy_id_text(hit.id)
        ));
        return;
    };
    let before = local.calls.hp_of(memory, enemy).map_or(0, |(hp, _)| hp);
    if before <= 0 {
        combat.counts.hits_refused += 1;
        log_line(&format!(
            "combat: guest hit on {} not applied (already dead)",
            enemy_id_text(hit.id)
        ));
        return;
    }
    if victim_refuses_hit(memory, enemy, hit.flags, is_boss(hit.id)) {
        combat.counts.hits_refused += 1;
        log_line(&format!(
            "combat: guest hit on {} not applied (the enemy is protected from hits right now, flags {:#x})",
            enemy_id_text(hit.id),
            hit.flags
        ));
        return;
    }
    let flags = hit.flags & GUEST_HIT_FLAGS_KEPT;
    let scaling = combat
        .actors
        .get(&hit.id)
        .and_then(|found| combat.damage_taken.scaling((enemy, found.serial)));
    let scaled = scaling.map(|(_, wanted)| guest_hit_damage(hit.hp_damage, wanted));
    let hp_damage = scaled.map_or(hit.hp_damage, |damage| damage.hp_damage);
    let stand_in = crate::kaine::stand_in_hit_damage(hit.id, hp_damage);
    let hp_damage = stand_in.map_or(hp_damage, |(_, scaled_down)| scaled_down);
    let mut scale_memory = InProcessMemory::new(local.build);
    let scale_before = local.calls.damage_taken_scale(&scale_memory, enemy);
    if let Some(damage) = scaled {
        local
            .calls
            .set_damage_taken_scale(&mut scale_memory, enemy, damage.scale);
    }
    let (kind, source_class) = unpack_hit_kind(hit.attack_kind);
    as_remote_with_kind(kind, source_class, || {
        local.calls.direct_hit(
            puppet,
            enemy,
            [hp_damage, hit.second_gauge, hit.third_gauge],
            flags,
        );
    });
    let scale_after = local.calls.damage_taken_scale(&scale_memory, enemy);
    if let Some(damage) = scaled {
        local
            .calls
            .set_damage_taken_scale(&mut scale_memory, enemy, damage.scale);
    }
    combat.counts.hits_applied += 1;
    let after = local.calls.hp_of(memory, enemy).map_or(0, |(hp, _)| hp);
    log_line(&format!(
        "combat: applied guest hit on {}: hp {} sent, flags {flags:#x}, enemy hp {before} -> {after}",
        enemy_id_text(hit.id),
        hit.hp_damage
    ));
    if let Some((beat, scaled_down)) = stand_in {
        log_line(&format!(
            "stand-in: guest hit on {} in beat {beat} scaled x{GUEST_HIT_SCALE_IN_A_STAND_IN_BEAT:.2}: hp {} sent, {scaled_down} applied, enemy hp {before} -> {after}",
            enemy_id_text(hit.id),
            hit.hp_damage
        ));
    }
    if let Some(GuestHitDamage { scale: wanted, .. }) = scaled {
        log_line(&format!(
            "combat: scaled guest hit on {}: hp {} sent at full strength, damage scale {} before, x{wanted:.3} for the hit, {} right after it, x{wanted:.3} put back",
            enemy_id_text(hit.id),
            hit.hp_damage,
            scale_text(scale_before),
            scale_text(scale_after)
        ));
    }
    if is_boss(hit.id) {
        remember_guest_boss_hit(hit);
        log_guest_boss_hit(
            local.build,
            enemy,
            hit,
            hp_damage,
            before,
            after,
            kind,
            source_class,
        );
        log_line(&format!(
            "boss damage: from guest, {} sent {} amount {}, hp left {after}",
            enemy_id_text(hit.id),
            hit.hp_damage,
            before - after
        ));
    }
    if after <= 0 {
        combat.guest_kills.insert(hit.id);
        let paid = local.game.playable_actor().ok().is_some_and(|player| {
            local
                .calls
                .send_actor_message(memory, player, PLAYER_MESSAGE_ENEMY_KILLED, enemy, 0)
        });
        log_line(&format!(
            "combat: the guest killed {}; your kill credit {}",
            enemy_id_text(hit.id),
            if paid { "paid" } else { "not paid (no player)" }
        ));
    }
}

fn scale_text(scale: Option<f32>) -> String {
    scale.map_or_else(|| "unreadable".to_owned(), |scale| format!("x{scale:.3}"))
}

pub(super) fn after_scan(
    combat: &mut CombatState,
    local: &LocalView<'_>,
    net: &NetThread,
    now_ms: u64,
    update: &mut RosterUpdate,
    remote: &RemotePlayer,
    options: SessionOptions,
) {
    let options = options.clamped();
    combat.enter_phase(local.phase);
    mark_deaths_that_left(combat, local, update);
    let departed: HashMap<EnemyId, (u64, i32)> = update
        .gone
        .iter()
        .filter_map(|(id, _)| {
            let enemy = combat.actors.get(id)?;
            Some((*id, (enemy.actor, enemy.serial)))
        })
        .collect();
    combat.remember(
        update
            .seen
            .iter()
            .map(|(id, sighting)| (*id, sighting.actor, sighting.spawn_serial)),
    );
    let downed = downed_status();
    let partner_down = downed.session_active && downed.partner == DownedState::Downed;
    let local_down = downed.session_active && downed.local == DownedState::Downed;
    let mut memory = InProcessMemory::new(local.build);
    let local_player = local.game.playable_actor().unwrap_or(0);
    let puppet = first_actor_address().unwrap_or(0);
    let bullet_target = (remote.state.phase == local.phase && !partner_down && local_player != 0)
        .then(|| puppet_bullet_target(&memory, puppet))
        .flatten();
    publish_view(CombatView {
        role: Some(EnemySyncRole::Host),
        phase: local.phase,
        enemies: combat.actor_ids(),
        local_player,
        puppet,
        aggro_targets: combat.aggro.targets(),
        enemy_damage_bonus_percent: damage_dealt_bonus_percent(
            options.enemy_damage_percent,
            EXTRA_PLAYERS_WITH_ONE_GUEST,
        ),
        bullet_target,
    });
    apply_enemy_health(combat, local, update, options);
    report_kills(combat, local, net, now_ms, update, &departed);
    send_bodies_gone(combat, local, net, now_ms);
    update_puppet_kind(combat, &mut memory, local_player, puppet, partner_down);
    if remote.state.phase != local.phase {
        return;
    }
    let host_in_menu = player_in_menu(local.build);
    let guest_in_menu = remote.state.activity == PartnerActivity::InMenu;
    log_aggro_menus(combat, host_in_menu, guest_in_menu);
    let players = AggroPlayers {
        host: (!local_down && !host_in_menu)
            .then(|| local.game.position().ok())
            .flatten()
            .map(|at| [at.x, at.y, at.z]),
        guest: (!partner_down && !guest_in_menu && puppet != 0).then_some(remote.state.position),
    };
    let alive: Vec<(EnemyId, [f32; 3])> = update
        .seen
        .iter()
        .filter(|(_, sighting)| sighting.hp > 0)
        .map(|(id, sighting): &(EnemyId, EnemySighting)| (*id, sighting.position))
        .collect();
    for (id, target) in combat.aggro.plan(now_ms, &alive, players) {
        let Some(enemy) = combat.actor_of(&memory, id) else {
            continue;
        };
        let target_actor = match target {
            AggroTarget::Guest => puppet,
            AggroTarget::Host => local_player,
        };
        if target_actor == 0 {
            continue;
        }
        let pushed = local
            .calls
            .add_aggro(&memory, enemy, target_actor, AGGRO_PUSH_WEIGHT);
        log_line(&format!(
            "combat: aggro {} -> {} ({})",
            enemy_id_text(id),
            match target {
                AggroTarget::Guest => "guest",
                AggroTarget::Host => "host",
            },
            if pushed {
                "pushed"
            } else {
                "not a human-type enemy, left to its own AI"
            }
        ));
    }
    combat.counts.aggro_on_guest = combat.aggro.pushed_to_guest();
}

fn mark_deaths_that_left(combat: &CombatState, local: &LocalView<'_>, update: &mut RosterUpdate) {
    let memory = local.game.memory();
    for (id, reason) in &mut update.gone {
        if *reason != EnemyGoneReason::Removed {
            continue;
        }
        // A dying enemy leaves its script group at once, so it vanishes while its last scan
        // still showed some HP; its own HP (or the guest's killing blow) says it died.
        let dead_here = combat
            .actor_of(memory, *id)
            .and_then(|actor| local.calls.hp_of(memory, actor))
            .is_some_and(|(hp, _)| hp <= 0);
        if dead_here || combat.guest_kills.contains(id) {
            *reason = EnemyGoneReason::Killed;
        }
    }
}

fn report_kills(
    combat: &mut CombatState,
    local: &LocalView<'_>,
    net: &NetThread,
    now_ms: u64,
    update: &RosterUpdate,
    departed: &HashMap<EnemyId, (u64, i32)>,
) {
    let seen: Vec<(EnemyId, i32)> = update
        .seen
        .iter()
        .map(|(id, sighting)| (*id, sighting.hp))
        .collect();
    let killed_and_gone: Vec<EnemyId> = update
        .gone
        .iter()
        .filter(|(_, reason)| *reason == EnemyGoneReason::Killed)
        .map(|(id, _)| *id)
        .collect();
    for id in combat.kills.observe(local.phase, &seen, &killed_and_gone) {
        let by_guest = combat.guest_kills.remove(&id);
        net.send_relayed(Message::EnemyKilled(EnemyKilled {
            phase: local.phase,
            id,
            by_guest,
        }));
        combat.counts.kills_sent += 1;
        let body = departed.get(&id).copied().or_else(|| {
            combat
                .actors
                .get(&id)
                .map(|enemy| (enemy.actor, enemy.serial))
        });
        if let Some((actor, serial)) = body {
            combat
                .host_bodies
                .track(local.phase, id, actor, serial, now_ms);
        }
        log_line(&format!(
            "combat: {} died (by {}), sent",
            enemy_id_text(id),
            if by_guest { "guest" } else { "host or world" }
        ));
    }
}

fn send_bodies_gone(combat: &mut CombatState, local: &LocalView<'_>, net: &NetThread, now_ms: u64) {
    let memory = local.game.memory();
    let gone = combat
        .host_bodies
        .gone(local.phase, now_ms, |actor, serial| {
            actor_is_live(memory, actor).unwrap_or(false)
                && memory
                    .read_i32(actor + ACTOR_SPAWN_SERIAL_OFFSET)
                    .is_ok_and(|now| now == serial)
        });
    for (id, since_death) in gone {
        net.send_relayed(Message::EnemyBodyGone(EnemyBodyGone {
            phase: local.phase,
            id,
        }));
        log_line(&format!(
            "combat: {}'s body is gone here {since_death} ms after its death, sent",
            enemy_id_text(id)
        ));
    }
}

fn puppet_bullet_target(memory: &InProcessMemory, puppet: u64) -> Option<PuppetBulletTarget> {
    if puppet == 0 || !actor_is_live(memory, puppet).unwrap_or(false) {
        return None;
    }
    Some(PuppetBulletTarget {
        handle: memory
            .read_array::<8>(puppet + ACTOR_SELF_HANDLE_OFFSET)
            .ok()?,
        position: read_actor_position(memory, puppet).ok()?,
    })
}

fn update_puppet_kind(
    combat: &mut CombatState,
    memory: &mut InProcessMemory,
    local_player: u64,
    puppet: u64,
    partner_down: bool,
) {
    if puppet == 0 || local_player == 0 {
        combat.puppet_kind = None;
        return;
    }
    if combat.puppet_kind.is_none_or(|kind| kind.actor != puppet) {
        let Some(original) = read_kind(memory, puppet) else {
            return;
        };
        combat.puppet_kind = Some(PuppetKind {
            actor: puppet,
            original,
        });
    }
    let Some(kind) = combat.puppet_kind else {
        return;
    };
    let Some(player_kind) = read_kind(memory, local_player) else {
        return;
    };
    // Bit 0x2 switches the actor tick to the player's call shape, which stops the body's motion
    // update; bit 0x4 is in every enemy mask that has 0x2, and in no player attack mask.
    let wanted = if partner_down {
        kind.original
    } else {
        (player_kind & !ACTOR_KIND_SHORT_TICK_BIT) | ACTOR_KIND_ENEMY_TARGET_STAND_IN_BIT
    };
    if read_kind(memory, puppet) == Some(wanted) {
        return;
    }
    if memory
        .write_i32(puppet + ACTOR_KIND_MASK_OFFSET, wanted)
        .is_ok()
    {
        log_line(&format!(
            "combat: guest body kind set to {wanted:#x} ({}; player kind {player_kind:#x}, body default {:#x})",
            if partner_down {
                "guest downed, enemies drop it"
            } else {
                "enemies can see it"
            },
            kind.original
        ));
    }
}

fn apply_enemy_health(
    combat: &mut CombatState,
    local: &LocalView<'_>,
    update: &RosterUpdate,
    options: SessionOptions,
) {
    let wanted = (options.enemy_health_percent, options.enemy_damage_percent);
    if combat.announced_difficulty != Some(wanted) {
        combat.announced_difficulty = Some(wanted);
        log_line(&format!(
            "difficulty: per extra player enemy health +{}%, enemy damage +{}% (one guest)",
            wanted.0, wanted.1
        ));
    }
    let mut memory = InProcessMemory::new(local.build);
    let ids: HashMap<ScaledEnemyKey, EnemyId> = update
        .seen
        .iter()
        .map(|(id, sighting)| ((sighting.actor, sighting.spawn_serial), *id))
        .collect();
    let current: Vec<(ScaledEnemyKey, f32)> = update
        .seen
        .iter()
        .filter_map(|(_, sighting)| {
            let scale = local.calls.damage_taken_scale(&memory, sighting.actor)?;
            Some(((sighting.actor, sighting.spawn_serial), scale))
        })
        .collect();
    let factor = damage_taken_factor(options.enemy_health_percent, EXTRA_PLAYERS_WITH_ONE_GUEST);
    for (key, scale) in combat.damage_taken.writes(&current, factor) {
        let written = local
            .calls
            .set_damage_taken_scale(&mut memory, key.0, scale);
        log_line(&format!(
            "difficulty: enemy {} takes hits at x{scale:.3} (health +{}%, {})",
            ids.get(&key)
                .map_or_else(|| "?".to_owned(), |id| enemy_id_text(*id)),
            options.enemy_health_percent,
            if written { "written" } else { "not written" }
        ));
    }
}

pub(super) fn restore_enemy_health(combat: &mut CombatState, build: &VerifiedBuild) {
    let mut memory = InProcessMemory::new(build);
    let calls = EnemyCalls::new(build);
    for (key, original) in combat.damage_taken.restores() {
        let still_it = actor_is_live(&memory, key.0).unwrap_or(false)
            && memory
                .read_i32(key.0 + ACTOR_SPAWN_SERIAL_OFFSET)
                .is_ok_and(|serial| serial == key.1);
        if still_it && calls.set_damage_taken_scale(&mut memory, key.0, original) {
            log_line(&format!(
                "difficulty: enemy at {:#x} takes hits at x{original:.3} again",
                key.0
            ));
        }
    }
}

fn log_aggro_menus(combat: &mut CombatState, host_in_menu: bool, guest_in_menu: bool) {
    if combat.aggro_menus_logged == (host_in_menu, guest_in_menu) {
        return;
    }
    combat.aggro_menus_logged = (host_in_menu, guest_in_menu);
    let text = match (host_in_menu, guest_in_menu) {
        (true, true) => "combat: aggro: host and guest both in a menu, enemies idle",
        (true, false) => "combat: aggro: host in menu, enemies go to the guest",
        (false, true) => "combat: aggro: guest in menu, enemies go to the host",
        (false, false) => "combat: aggro: nobody in a menu, enemies pick the nearer player",
    };
    log_line(text);
}
