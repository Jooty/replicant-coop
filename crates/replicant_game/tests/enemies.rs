use replicant_game::enemies::{
    ENEMY_MIRROR_STALE_MS, EnemyMirror, EnemyRoster, EnemySighting, enemy_state,
    live_companion_texts, match_enemies, partner_actors, scan_enemies, shadow_matrix,
    split_snapshot,
};
use replicant_game::offsets::{
    ACTOR_CLASS_ID_OFFSET, ACTOR_KIND_MASK_OFFSET, ACTOR_MANAGER_GROUP_COUNT_OFFSET,
    ACTOR_MANAGER_GROUP_STRIDE, ACTOR_MANAGER_RVA, ACTOR_SELF_HANDLE_OFFSET,
    ACTOR_SPAWN_SERIAL_OFFSET, ACTOR_TRANSFORM_BLOCK_BYTES, ACTOR_TRANSFORM_BLOCK_OFFSET,
    HANDLE_POOL_GENERATIONS_RVA, HANDLE_POOL_POINTERS_RVA, PARTNER_KAINE_ACTOR_RVA,
    PARTNER_THIRD_MEMBER_ACTOR_RVA, PARTNER_VARIANT_POINTER_RVA, PUPPET_ACTOR_GROUP,
};
use replicant_game::puppet::PuppetPose;
use replicant_game::{GameMemory, Position};
use replicant_proto::{
    ENEMY_GROUP_COUNT, EnemyGone, EnemyGoneReason, EnemyId, EnemySnapshot, EnemySpawned,
    EnemyState, MAX_ENEMIES_PER_SNAPSHOT, MAX_ENEMY_SNAPSHOT_PARTS, Message, encode,
};

use crate::fake_memory::FakeMemory;

const PHASE: u16 = 12;

fn sighting(actor: u64, group: u16, serial: i32) -> EnemySighting {
    EnemySighting {
        actor,
        spawn_serial: serial,
        group,
        class_id: 0,
        position: [actor as f32, 0.0, 0.0],
        yaw_radians: 0.5,
        motion: None,
        hp: 50,
        max_hp: 50,
    }
}

fn id(group: u16, ordinal: u16) -> EnemyId {
    EnemyId { group, ordinal }
}

fn state(group: u16, ordinal: u16, x: f32) -> EnemyState {
    EnemyState {
        id: id(group, ordinal),
        position: [x, 0.0, 0.0],
        yaw_radians: 0.0,
        motion: None,
        hp: 10,
        max_hp: 20,
    }
}

fn snapshot(tick: u32, enemies: Vec<EnemyState>) -> EnemySnapshot {
    EnemySnapshot {
        tick,
        phase: PHASE,
        part: 0,
        parts: 1,
        total: u16::try_from(enemies.len()).expect("small"),
        round: u16::try_from(enemies.len()).expect("small"),
        enemies,
    }
}

#[test]
fn ordinals_follow_spawn_order_within_a_group_not_scan_order() {
    let mut roster = EnemyRoster::new();
    let update = roster.observe(
        PHASE,
        &[
            sighting(0xC000, 7, 30),
            sighting(0xA000, 7, 10),
            sighting(0xB000, 7, 20),
            sighting(0xD000, 9, 5),
        ],
    );
    let ids: Vec<(EnemyId, u64)> = update
        .seen
        .iter()
        .map(|(id, sighting)| (*id, sighting.actor))
        .collect();
    assert_eq!(
        ids,
        vec![
            (id(7, 0), 0xA000),
            (id(7, 1), 0xB000),
            (id(7, 2), 0xC000),
            (id(9, 0), 0xD000),
        ]
    );
    assert_eq!(update.spawned.len(), 4);
}

#[test]
fn two_machines_with_different_addresses_and_serials_agree_on_the_ids() {
    let mut host = EnemyRoster::new();
    let mut guest = EnemyRoster::new();
    let host_ids: Vec<EnemyId> = host
        .observe(PHASE, &[sighting(0x100, 3, 900), sighting(0x200, 3, 901)])
        .spawned;
    let guest_ids: Vec<EnemyId> = guest
        .observe(PHASE, &[sighting(0x7700, 3, 12), sighting(0x6600, 3, 14)])
        .spawned;
    assert_eq!(host_ids, guest_ids);
}

#[test]
fn an_enemy_keeps_its_id_while_others_come_and_go() {
    let mut roster = EnemyRoster::new();
    roster.observe(PHASE, &[sighting(0xA, 4, 1), sighting(0xB, 4, 2)]);
    let update = roster.observe(PHASE, &[sighting(0xB, 4, 2), sighting(0xC, 4, 3)]);
    assert_eq!(update.spawned, vec![id(4, 2)]);
    assert_eq!(update.gone, vec![(id(4, 0), EnemyGoneReason::Removed)]);
    let seen: Vec<EnemyId> = update.seen.iter().map(|(id, _)| *id).collect();
    assert_eq!(seen, vec![id(4, 1), id(4, 2)]);
}

#[test]
fn an_enemy_last_seen_at_zero_hp_is_reported_killed() {
    let mut roster = EnemyRoster::new();
    roster.observe(PHASE, &[sighting(0xA, 4, 1)]);
    let dying = EnemySighting {
        hp: 0,
        ..sighting(0xA, 4, 1)
    };
    roster.observe(PHASE, &[dying]);
    let update = roster.observe(PHASE, &[]);
    assert_eq!(update.gone, vec![(id(4, 0), EnemyGoneReason::Killed)]);
}

#[test]
fn a_reused_address_with_a_new_serial_is_a_new_enemy() {
    let mut roster = EnemyRoster::new();
    roster.observe(PHASE, &[sighting(0xA, 4, 1)]);
    let update = roster.observe(PHASE, &[sighting(0xA, 4, 2)]);
    assert_eq!(update.spawned, vec![id(4, 1)]);
    assert_eq!(update.gone, vec![(id(4, 0), EnemyGoneReason::Removed)]);
}

#[test]
fn a_new_phase_numbers_from_zero_again_without_death_events() {
    let mut roster = EnemyRoster::new();
    roster.observe(PHASE, &[sighting(0xA, 4, 1), sighting(0xB, 4, 2)]);
    let update = roster.observe(PHASE + 1, &[sighting(0xC, 4, 3)]);
    assert!(update.phase_changed);
    assert_eq!(update.spawned, vec![id(4, 0)]);
    assert!(update.gone.is_empty());
}

#[test]
fn an_enemy_of_the_last_phase_still_loaded_gets_no_id_in_the_new_phase() {
    let mut host = EnemyRoster::new();
    let mut guest = EnemyRoster::new();
    host.observe(PHASE, &[sighting(0xA, 100, 1)]);
    guest.observe(PHASE, &[sighting(0x7A, 100, 5)]);
    let host_change = host.observe(PHASE + 1, &[]);
    let guest_change = guest.observe(PHASE + 1, &[sighting(0x7A, 100, 5)]);
    assert_eq!(guest_change.unloaded, vec![id(100, 0)]);
    assert!(guest_change.spawned.is_empty());
    assert!(guest_change.seen.is_empty());
    assert_eq!(host_change.last_phase_leftovers_found, 0);
    assert_eq!(guest_change.last_phase_leftovers_found, 1);
    let guest_unload = guest.observe(PHASE + 1, &[]);
    assert!(guest_unload.gone.is_empty());
    assert_eq!(guest_unload.last_phase_leftovers_gone, 1);
    let host_boss = host.observe(PHASE + 1, &[sighting(0xB, 100, 2)]);
    let guest_boss = guest.observe(PHASE + 1, &[sighting(0x7B, 100, 9)]);
    assert_eq!(host_boss.spawned, vec![id(100, 0)]);
    assert_eq!(guest_boss.spawned, host_boss.spawned);
    assert_eq!(guest.own_arrival_of(id(100, 0)), Some(0));
}

#[test]
fn a_new_enemy_beside_a_leftover_of_the_last_phase_still_counts_from_zero() {
    let mut roster = EnemyRoster::new();
    roster.observe(PHASE, &[sighting(0xA, 4, 1)]);
    let update = roster.observe(PHASE + 1, &[sighting(0xA, 4, 1), sighting(0xB, 4, 2)]);
    assert_eq!(update.spawned, vec![id(4, 0)]);
    let seen: Vec<u64> = update.seen.iter().map(|(_, seen)| seen.actor).collect();
    assert_eq!(seen, vec![0xB]);
}

#[test]
fn a_leftover_stays_without_an_id_through_a_second_phase_change() {
    let mut roster = EnemyRoster::new();
    roster.observe(PHASE, &[sighting(0xA, 4, 1)]);
    roster.observe(PHASE + 1, &[sighting(0xA, 4, 1)]);
    let update = roster.observe(PHASE + 2, &[sighting(0xA, 4, 1)]);
    assert!(update.spawned.is_empty());
    assert_eq!(update.last_phase_leftovers_found, 1);
}

#[test]
fn a_reused_address_after_a_leftover_went_is_a_new_enemy() {
    let mut roster = EnemyRoster::new();
    roster.observe(PHASE, &[sighting(0xA, 4, 1)]);
    roster.observe(PHASE + 1, &[sighting(0xA, 4, 1)]);
    let update = roster.observe(PHASE + 1, &[sighting(0xA, 4, 2)]);
    assert_eq!(update.spawned, vec![id(4, 0)]);
    assert_eq!(update.last_phase_leftovers_gone, 1);
}

#[test]
fn a_sighting_becomes_a_valid_wire_state() {
    let odd = EnemySighting {
        yaw_radians: 7.0,
        hp: 80,
        max_hp: 60,
        ..sighting(0xA, 4, 1)
    };
    let wire = enemy_state(id(4, 0), &odd).expect("finite position");
    assert!(wire.yaw_radians.abs() <= std::f32::consts::PI);
    assert_eq!((wire.hp, wire.max_hp), (60, 60));
    let spawned = Message::EnemySpawned(EnemySpawned {
        phase: PHASE,
        enemy: wire,
    });
    assert!(encode(&spawned).is_ok());
    let lost = EnemySighting {
        position: [f32::NAN, 0.0, 0.0],
        ..sighting(0xA, 4, 1)
    };
    assert_eq!(enemy_state(id(4, 0), &lost), None);
}

#[test]
fn a_big_snapshot_is_split_into_parts_that_each_encode() {
    let enemies: Vec<EnemyState> = (0..12).map(|n| state(1, n, f32::from(n))).collect();
    let parts = split_snapshot(5, PHASE, enemies, &[[0.0; 3]]);
    assert_eq!(parts.len(), 3);
    for (index, part) in parts.iter().enumerate() {
        assert_eq!(usize::from(part.part), index);
        assert_eq!(part.parts, 3);
        assert_eq!(part.total, 12);
        assert!(encode(&Message::EnemySnapshot(part.clone())).is_ok());
    }
}

#[test]
fn when_too_many_enemies_the_nearest_to_either_player_are_sent() {
    let limit = MAX_ENEMIES_PER_SNAPSHOT * usize::from(MAX_ENEMY_SNAPSHOT_PARTS);
    let count = u16::try_from(limit + 3).expect("small");
    let enemies: Vec<EnemyState> = (0..count).map(|n| state(1, n, f32::from(n))).collect();
    let far_player = [f32::from(count - 1), 0.0, 0.0];
    let parts = split_snapshot(5, PHASE, enemies, &[[-1000.0, 0.0, 0.0], far_player]);
    let sent: Vec<u16> = parts
        .iter()
        .flat_map(|part| part.enemies.iter().map(|enemy| enemy.id.ordinal))
        .collect();
    assert_eq!(sent.len(), limit);
    assert_eq!(sent[0], count - 1);
    assert!(!sent.contains(&0));
    assert_eq!(parts[0].total, count);
    assert_eq!(usize::from(parts[0].round), limit);
}

#[test]
fn no_enemies_still_sends_one_empty_part() {
    let parts = split_snapshot(5, PHASE, Vec::new(), &[]);
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].total, 0);
}

#[test]
fn the_mirror_interpolates_between_snapshots() {
    let mut mirror = EnemyMirror::new();
    mirror.receive_snapshot(0, &snapshot(1, vec![state(2, 0, 0.0)]));
    mirror.receive_snapshot(100, &snapshot(2, vec![state(2, 0, 10.0)]));
    let sampled = mirror.sample(150, 100);
    assert_eq!(sampled.len(), 1);
    assert!((sampled[0].position[0] - 5.0).abs() < 1e-4);
    assert_eq!(mirror.host_total(), Some(1));
}

#[test]
fn a_dead_enemy_is_not_brought_back_by_a_late_snapshot() {
    let mut mirror = EnemyMirror::new();
    mirror.receive_snapshot(0, &snapshot(1, vec![state(2, 0, 0.0), state(2, 1, 0.0)]));
    mirror.receive_gone(&EnemyGone {
        phase: PHASE,
        id: id(2, 0),
        reason: EnemyGoneReason::Killed,
    });
    mirror.receive_snapshot(10, &snapshot(1, vec![state(2, 0, 0.0), state(2, 1, 0.0)]));
    let ids: Vec<EnemyId> = mirror.sample(10, 0).iter().map(|enemy| enemy.id).collect();
    assert_eq!(ids, vec![id(2, 1)]);
}

#[test]
fn an_enemy_the_host_stops_sending_is_dropped() {
    let mut mirror = EnemyMirror::new();
    mirror.receive_snapshot(0, &snapshot(1, vec![state(2, 0, 0.0)]));
    mirror.prune(ENEMY_MIRROR_STALE_MS);
    assert_eq!(mirror.len(), 1);
    mirror.prune(ENEMY_MIRROR_STALE_MS + 1);
    assert!(mirror.is_empty());
}

#[test]
fn a_message_from_a_new_phase_clears_the_mirror() {
    let mut mirror = EnemyMirror::new();
    mirror.receive_snapshot(0, &snapshot(1, vec![state(2, 0, 0.0)]));
    mirror.receive_spawned(
        5,
        &EnemySpawned {
            phase: PHASE + 1,
            enemy: state(8, 0, 1.0),
        },
    );
    assert_eq!(mirror.phase(), Some(PHASE + 1));
    let ids: Vec<EnemyId> = mirror.sample(5, 0).iter().map(|enemy| enemy.id).collect();
    assert_eq!(ids, vec![id(8, 0)]);
}

#[test]
fn matching_reports_pairs_strays_and_the_worst_error() {
    let local = [
        (id(1, 0), [0.0, 0.0, 0.0]),
        (id(1, 1), [0.0, 0.0, 0.0]),
        (id(3, 0), [0.0, 0.0, 0.0]),
    ];
    let remote = [
        (id(1, 0), [3.0, 4.0, 0.0]),
        (id(1, 1), [1.0, 0.0, 0.0]),
        (id(5, 0), [0.0, 0.0, 0.0]),
    ];
    let report = match_enemies(&local, &remote);
    assert_eq!((report.local_count, report.remote_count), (3, 3));
    assert_eq!(report.matched.len(), 2);
    assert_eq!(report.unmatched_local, vec![id(3, 0)]);
    assert_eq!(report.unmatched_remote, vec![id(5, 0)]);
    assert_eq!(report.worst_error(), Some((id(1, 0), 5.0)));
}

#[test]
fn the_shadow_matrix_keeps_the_enemys_scale_and_takes_the_hosts_pose() {
    let big = PuppetPose {
        position: Position {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        },
        yaw: 0.3,
    }
    .creation_matrix();
    let mut block = [0_u8; ACTOR_TRANSFORM_BLOCK_BYTES];
    block.copy_from_slice(&big[..ACTOR_TRANSFORM_BLOCK_BYTES]);
    for lane in [0, 2, 5, 8, 10] {
        let at = lane * 4;
        let value = f32::from_le_bytes(block[at..at + 4].try_into().expect("4 bytes")) * 2.5;
        block[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    let matrix = shadow_matrix(&block, [10.0, 20.0, 30.0], 1.2);
    let mut moved = [0_u8; ACTOR_TRANSFORM_BLOCK_BYTES];
    moved.copy_from_slice(&matrix[..ACTOR_TRANSFORM_BLOCK_BYTES]);
    let pose = PuppetPose::from_transform_block(&moved);
    assert_eq!(
        pose.position,
        Position {
            x: 10.0,
            y: 20.0,
            z: 30.0
        }
    );
    assert!((pose.yaw - 1.2).abs() < 1e-5);
    let middle = f32::from_le_bytes(matrix[20..24].try_into().expect("4 bytes"));
    assert!((middle - 2.5).abs() < 1e-5);
}

const ENEMY: u64 = 0x5_0000_0000;
const PLAYER: u64 = 0x6_0000_0000;
const PUPPET: u64 = 0x7_0000_0000;
const PROP: u64 = 0x8_0000_0000;

fn put_actor(memory: &mut FakeMemory, actor: u64, slot: u32, kind_mask: u32, serial: i32) {
    memory.put(actor + ACTOR_KIND_MASK_OFFSET, &kind_mask.to_le_bytes());
    let mut handle = [0_u8; 8];
    handle[..4].copy_from_slice(&slot.to_le_bytes());
    handle[4..].copy_from_slice(&1_u32.to_le_bytes());
    memory.put(actor + ACTOR_SELF_HANDLE_OFFSET, &handle);
    memory.put_at_rva(HANDLE_POOL_POINTERS_RVA + slot * 8, &actor.to_le_bytes());
    memory.put_at_rva(HANDLE_POOL_GENERATIONS_RVA + slot * 4, &1_u32.to_le_bytes());
    memory.put(actor + ACTOR_SPAWN_SERIAL_OFFSET, &serial.to_le_bytes());
    let pose = PuppetPose {
        position: Position {
            x: 4.0,
            y: 5.0,
            z: 6.0,
        },
        yaw: 0.25,
    };
    memory.put(
        actor + ACTOR_TRANSFORM_BLOCK_OFFSET,
        &pose.creation_matrix()[..ACTOR_TRANSFORM_BLOCK_BYTES],
    );
}

fn set_group_count(memory: &mut FakeMemory, group: u16, count: i32) {
    let offset = ACTOR_MANAGER_GROUP_COUNT_OFFSET + u64::from(group) * ACTOR_MANAGER_GROUP_STRIDE;
    let at = memory.module_base() + u64::from(ACTOR_MANAGER_RVA) + offset;
    memory.put(at, &count.to_le_bytes());
}

#[test]
fn the_scan_keeps_live_enemies_and_skips_the_player_puppets_and_props() {
    let mut memory = FakeMemory::new();
    let table_bytes = usize::from(ENEMY_GROUP_COUNT) * ACTOR_MANAGER_GROUP_STRIDE as usize;
    memory.put_at_rva(
        u32::try_from(u64::from(ACTOR_MANAGER_RVA) + ACTOR_MANAGER_GROUP_COUNT_OFFSET)
            .expect("rva"),
        &vec![0_u8; table_bytes],
    );
    put_actor(&mut memory, ENEMY, 1, 0x2, 77);
    put_actor(&mut memory, PLAYER, 2, 0x2, 1);
    put_actor(&mut memory, PUPPET, 3, 0x2, 2);
    put_actor(&mut memory, PROP, 4, 0x100, 3);
    let puppet_group = u16::try_from(PUPPET_ACTOR_GROUP).expect("group");
    set_group_count(&mut memory, 6, 3);
    set_group_count(&mut memory, puppet_group, 1);
    let found = scan_enemies(
        &memory,
        Some(PLAYER),
        &[],
        |group, index| match (group, index) {
            (6, 0) => ENEMY,
            (6, 1) => PLAYER,
            (6, 2) => PROP,
            (group, 0) if group == puppet_group => PUPPET,
            _ => 0,
        },
        |actor| (actor == ENEMY).then_some((30, 40)),
    )
    .expect("scan");
    assert_eq!(found.len(), 1);
    let enemy = found[0];
    assert_eq!(
        (enemy.actor, enemy.group, enemy.spawn_serial),
        (ENEMY, 6, 77)
    );
    assert_eq!(enemy.position, [4.0, 5.0, 6.0]);
    assert!((enemy.yaw_radians - 0.25).abs() < 1e-5);
    assert_eq!((enemy.hp, enemy.max_hp), (30, 40));
}

#[test]
fn the_companions_are_the_two_static_partners_and_the_variant_when_it_is_set() {
    let mut memory = FakeMemory::new();
    let base = memory.module_base();
    let kaine = base + u64::from(PARTNER_KAINE_ACTOR_RVA);
    let third = base + u64::from(PARTNER_THIRD_MEMBER_ACTOR_RVA);
    assert_eq!(partner_actors(&memory), vec![kaine, third]);
    memory.put_at_rva(PARTNER_VARIANT_POINTER_RVA, &ENEMY.to_le_bytes());
    assert_eq!(partner_actors(&memory), vec![kaine, third, ENEMY]);
}

#[test]
fn only_a_live_companion_is_described_for_the_log() {
    let mut memory = FakeMemory::new();
    assert!(live_companion_texts(&memory).is_empty());
    let kaine = memory.module_base() + u64::from(PARTNER_KAINE_ACTOR_RVA);
    put_actor(&mut memory, kaine, 9, 0x2, 5);
    memory.put(kaine + ACTOR_CLASS_ID_OFFSET, &0x51_i32.to_le_bytes());
    let texts = live_companion_texts(&memory);
    assert_eq!(texts.len(), 1);
    assert!(
        texts[0].starts_with("unique actor -2 class 0x51 at (4.0, 5.0, 6.0) yaw 0.25, "),
        "{}",
        texts[0]
    );
}

#[test]
fn the_scan_leaves_out_the_companion_like_the_player() {
    let mut memory = FakeMemory::new();
    let table_bytes = usize::from(ENEMY_GROUP_COUNT) * ACTOR_MANAGER_GROUP_STRIDE as usize;
    memory.put_at_rva(
        u32::try_from(u64::from(ACTOR_MANAGER_RVA) + ACTOR_MANAGER_GROUP_COUNT_OFFSET)
            .expect("rva"),
        &vec![0_u8; table_bytes],
    );
    put_actor(&mut memory, ENEMY, 1, 0x2, 77);
    put_actor(&mut memory, PROP, 4, 0x2, 3);
    set_group_count(&mut memory, 6, 2);
    let found = scan_enemies(
        &memory,
        None,
        &[PROP],
        |group, index| match (group, index) {
            (6, 0) => ENEMY,
            (6, 1) => PROP,
            _ => 0,
        },
        |_| Some((30, 40)),
    )
    .expect("scan");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].actor, ENEMY);
}

#[test]
fn a_boss_started_a_second_time_takes_the_hosts_id_and_keeps_it() {
    let mut guest = EnemyRoster::new();
    let mut update = guest.observe(25, &[sighting(0xB055, 1001, 1)]);
    assert_eq!(update.spawned, vec![id(1001, 0)]);
    let adoptions = guest.adopt_host_ids(&update.spawned, &[id(1001, 1), id(23, 0)]);
    assert_eq!(adoptions, vec![(id(1001, 0), id(1001, 1))]);
    update.rename(&adoptions);
    assert_eq!(update.spawned, vec![id(1001, 1)]);
    assert_eq!(update.seen[0].0, id(1001, 1));
    let next = guest.observe(25, &[sighting(0xB055, 1001, 1)]);
    assert_eq!(next.seen[0].0, id(1001, 1));
    let another = guest.observe(25, &[sighting(0xB055, 1001, 1), sighting(0xB056, 1001, 2)]);
    assert_eq!(another.spawned, vec![id(1001, 2)]);
}

#[test]
fn a_new_enemy_the_host_already_has_or_has_no_group_for_keeps_its_own_id() {
    let mut guest = EnemyRoster::new();
    let update = guest.observe(25, &[sighting(1, 23, 1), sighting(2, 24, 2)]);
    assert!(
        guest
            .adopt_host_ids(&update.spawned, &[id(23, 0), id(30, 4)])
            .is_empty()
    );
}

#[test]
fn a_host_id_already_held_here_is_not_adopted_twice() {
    let mut guest = EnemyRoster::new();
    guest.observe(25, &[sighting(1, 23, 1)]);
    let update = guest.observe(25, &[sighting(1, 23, 1), sighting(2, 23, 2)]);
    assert_eq!(update.spawned, vec![id(23, 1)]);
    assert!(
        guest
            .adopt_host_ids(&update.spawned, &[id(23, 0), id(23, 1)])
            .is_empty()
    );
    let third = guest.observe(
        25,
        &[sighting(1, 23, 1), sighting(2, 23, 2), sighting(3, 23, 3)],
    );
    assert_eq!(
        guest.adopt_host_ids(&third.spawned, &[id(23, 0), id(23, 1), id(23, 5)]),
        vec![(id(23, 2), id(23, 5))]
    );
}
