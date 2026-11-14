use std::collections::HashSet;

use replicant_game::desync::ids_hash;
use replicant_game::enemies::{
    EnemyRoster, EnemySighting, FIGHT_GUEST_ONLY_ENEMY_GRACE_MS, GuestOnlyWatch, HostFightEnemy,
    HostHas, HostListView, WaitingFightSpawn, plan_fight_bindings,
};
use replicant_proto::EnemyId;

const PHASE: u16 = 16;
const WAVE_GROUP: u16 = 7;
const BIG_CLASS: i32 = 0x25;
const SMALL_CLASS: i32 = 0x23;
const BIG_MAX_HP: i32 = 20;
const SMALL_MAX_HP: i32 = 15;
const BURST_MAX_HP: i32 = 2;
const BURST_GROUP: u16 = 5;
const GROUP_7_BIG_SPAWN: [f32; 3] = [11.7, 69.6, -11.7];
const GROUP_7_SMALL_SPAWN: [f32; 3] = [11.7, 65.4, -11.7];
const BURST_SPAWNS: [[f32; 3]; 8] = [
    [0.0, 69.6, -16.5],
    [11.7, 69.6, -11.7],
    [16.5, 69.6, 0.0],
    [11.7, 69.6, 11.7],
    [0.0, 69.6, 16.5],
    [-11.7, 69.6, 11.7],
    [-16.5, 69.6, 0.0],
    [-11.7, 69.6, -11.7],
];
const GUEST_ONLY_GROUPS: [u16; 4] = [5, 7, 9, 11];
const GUEST_ONLY_IDS_HASH: u32 = 0xc040_9be0;

fn id(group: u16, ordinal: u16) -> EnemyId {
    EnemyId { group, ordinal }
}

fn sighting(
    actor: u64,
    group: u16,
    class_id: i32,
    max_hp: i32,
    position: [f32; 3],
) -> EnemySighting {
    EnemySighting {
        actor,
        spawn_serial: i32::try_from(actor).unwrap(),
        group,
        class_id,
        position,
        yaw_radians: 0.0,
        motion: None,
        hp: max_hp,
        max_hp,
    }
}

fn host(id: EnemyId, max_hp: i32, position: [f32; 3]) -> HostFightEnemy {
    HostFightEnemy {
        id,
        max_hp: max_hp.cast_unsigned(),
        position,
    }
}

fn waiting(roster: &EnemyRoster, seen: &[(EnemyId, EnemySighting)]) -> Vec<WaitingFightSpawn> {
    roster
        .fight_waiting_spawns()
        .into_iter()
        .filter_map(|waiting| {
            let (_, sighting) = seen.iter().find(|(seen_id, _)| *seen_id == waiting)?;
            Some(WaitingFightSpawn {
                id: waiting,
                max_hp: sighting.max_hp,
                position: sighting.position,
            })
        })
        .collect()
}

fn others_than(roster: &EnemyRoster, waiting: &[WaitingFightSpawn]) -> HashSet<EnemyId> {
    roster
        .held_ids()
        .into_iter()
        .filter(|held| !waiting.iter().any(|spawn| spawn.id == *held))
        .collect()
}

#[test]
fn a_wave_the_guests_game_skipped_does_not_shift_its_later_spawns_onto_killed_ids() {
    // run 1113b G4489-4725: the guest's game never made the host's 15-hp 7:1, a copy filled in
    // and was killed; the guest's next own spawn (20 hp) was bound to the dead 7:1 by its order.
    let mut roster = EnemyRoster::new();
    let own_first = sighting(1, WAVE_GROUP, BIG_CLASS, BIG_MAX_HP, GROUP_7_BIG_SPAWN);
    let update = roster.observe(PHASE, &[own_first]);
    assert_eq!(update.spawned, vec![id(WAVE_GROUP, 0)]);
    roster.stop_all_waiting();
    let copy = sighting(
        2,
        WAVE_GROUP,
        SMALL_CLASS,
        SMALL_MAX_HP,
        GROUP_7_SMALL_SPAWN,
    );
    roster.claim(
        copy.actor,
        copy.spawn_serial,
        id(WAVE_GROUP, 1),
        SMALL_MAX_HP,
    );
    roster.observe(PHASE, &[own_first, copy]);
    roster.observe(PHASE, &[]);
    let ended: HashSet<EnemyId> = [id(WAVE_GROUP, 0), id(WAVE_GROUP, 1)].into_iter().collect();
    let host_live = [
        host(id(WAVE_GROUP, 2), BIG_MAX_HP, GROUP_7_BIG_SPAWN),
        host(id(WAVE_GROUP, 3), SMALL_MAX_HP, GROUP_7_SMALL_SPAWN),
    ];
    let own_big = sighting(3, WAVE_GROUP, BIG_CLASS, BIG_MAX_HP, GROUP_7_BIG_SPAWN);
    let own_small = sighting(4, WAVE_GROUP, 0x22, SMALL_MAX_HP, [11.7, 65.3, -11.7]);
    let update = roster.observe(PHASE, &[own_big, own_small]);
    let own_ids = update.spawned.clone();
    assert_eq!(
        roster.own_arrival_of(own_ids[0]),
        Some(1),
        "by order alone the 20-hp spawn would be the killed 15-hp 7:1"
    );
    let waiting = waiting(&roster, &update.seen);
    let plan = plan_fight_bindings(
        &waiting,
        &host_live,
        &ended,
        &others_than(&roster, &waiting),
        &HashSet::new(),
        &HashSet::new(),
    );
    let mut bound: Vec<EnemyId> = plan.kept.clone();
    bound.extend(plan.renames.iter().map(|(_, to)| *to));
    bound.sort();
    assert_eq!(bound, vec![id(WAVE_GROUP, 2), id(WAVE_GROUP, 3)]);
    assert!(plan.takeovers.is_empty());
    assert!(plan.retired.is_empty());
    let mut pairs = plan.renames.clone();
    pairs.extend(plan.kept.iter().map(|kept| (*kept, *kept)));
    for (own, to) in pairs {
        let (_, own_sighting) = update.seen.iter().find(|(seen, _)| *seen == own).unwrap();
        let host = host_live.iter().find(|host| host.id == to).unwrap();
        assert_eq!(
            own_sighting.max_hp.cast_unsigned(),
            host.max_hp,
            "{own:?} pairs with a host enemy of its own max hp"
        );
    }
}

#[test]
fn eight_host_spawns_in_one_frame_each_pair_with_the_own_spawn_at_their_spot() {
    // run 1113b H4035-4042: the host spawned 5:6..5:13 (2 hp) in one frame while 5:4 (20 hp) lived.
    let mut host_live: Vec<HostFightEnemy> = BURST_SPAWNS
        .iter()
        .zip(6_u16..)
        .map(|(at, ordinal)| host(id(BURST_GROUP, ordinal), BURST_MAX_HP, *at))
        .collect();
    host_live.push(host(id(BURST_GROUP, 4), BIG_MAX_HP, BURST_SPAWNS[0]));
    let mut roster = EnemyRoster::new();
    roster.observe(PHASE, &[]);
    for ordinal in 0..5 {
        roster.claim(100 + u64::from(ordinal), 0, id(BURST_GROUP, ordinal), 1);
    }
    roster.observe(PHASE, &[]);
    let own: Vec<EnemySighting> = BURST_SPAWNS
        .iter()
        .rev()
        .zip(10_u64..)
        .map(|(at, actor)| sighting(actor, BURST_GROUP, BIG_CLASS, BURST_MAX_HP, *at))
        .collect();
    let mut update = roster.observe(PHASE, &own);
    assert_eq!(update.spawned.len(), BURST_SPAWNS.len());
    let waiting = waiting(&roster, &update.seen);
    let plan = plan_fight_bindings(
        &waiting,
        &host_live,
        &HashSet::new(),
        &others_than(&roster, &waiting),
        &HashSet::new(),
        &HashSet::new(),
    );
    assert!(plan.retired.is_empty());
    let mut moves = plan.renames.clone();
    moves.extend(plan.takeovers.iter().copied());
    let made = roster.reassign_ids(&moves);
    assert_eq!(made.len(), moves.len());
    update.rename(&made);
    let mut ids: Vec<EnemyId> = update.seen.iter().map(|(seen, _)| *seen).collect();
    ids.sort();
    assert_eq!(
        ids,
        (6..14)
            .map(|ordinal| id(BURST_GROUP, ordinal))
            .collect::<Vec<_>>()
    );
    for (seen, own_sighting) in &update.seen {
        let host = host_live.iter().find(|host| host.id == *seen).unwrap();
        assert_eq!(own_sighting.position, host.position, "{seen:?}");
    }
}

#[test]
fn a_guest_only_enemy_in_four_groups_is_retired_then_removed_after_the_fight_grace() {
    // run 1113b G6181-G13483: the guest kept one 20-hp enemy (ids hash c0409be0) in groups 5, 7,
    // 9 and 11, each on an id the host had killed (x:3), from 09:55:31 to the end.
    assert_eq!(ids_hash(&[BIG_CLASS]), GUEST_ONLY_IDS_HASH);
    let mut roster = EnemyRoster::new();
    roster.observe(PHASE, &[]);
    for group in GUEST_ONLY_GROUPS {
        for ordinal in 0..3 {
            roster.claim(
                u64::from(group) * 100 + u64::from(ordinal),
                0,
                id(group, ordinal),
                1,
            );
        }
    }
    roster.observe(PHASE, &[]);
    let own: Vec<EnemySighting> = GUEST_ONLY_GROUPS
        .iter()
        .zip(1_u64..)
        .map(|(group, actor)| sighting(actor, *group, BIG_CLASS, BIG_MAX_HP, [0.0; 3]))
        .collect();
    let mut update = roster.observe(PHASE, &own);
    let ended: HashSet<EnemyId> = GUEST_ONLY_GROUPS
        .iter()
        .map(|group| id(*group, 3))
        .collect();
    assert_eq!(
        update.spawned.iter().copied().collect::<HashSet<_>>(),
        ended,
        "each own spawn holds the id the host ended"
    );
    let host_live: Vec<HostFightEnemy> = (6..14)
        .map(|ordinal| host(id(BURST_GROUP, ordinal), BURST_MAX_HP, [0.0; 3]))
        .collect();
    let waiting = waiting(&roster, &update.seen);
    let plan = plan_fight_bindings(
        &waiting,
        &host_live,
        &ended,
        &others_than(&roster, &waiting),
        &HashSet::new(),
        &HashSet::new(),
    );
    assert!(plan.kept.is_empty() && plan.renames.is_empty() && plan.takeovers.is_empty());
    let mut retired_ids = plan.retired.clone();
    retired_ids.sort();
    let mut expected: Vec<EnemyId> = ended.iter().copied().collect();
    expected.sort();
    assert_eq!(retired_ids, expected);
    let moves: Vec<(EnemyId, EnemyId)> = plan
        .retired
        .iter()
        .map(|own| (*own, roster.retired_id(own.group)))
        .collect();
    let made = roster.reassign_ids(&moves);
    assert_eq!(made.len(), GUEST_ONLY_GROUPS.len());
    update.rename(&made);
    let complete_round = HostListView {
        round_covers_host: true,
        listed: None,
    };
    let guest_only: Vec<u64> = update
        .seen
        .iter()
        .filter(|(seen, _)| {
            !ended.contains(seen)
                && !host_live.iter().any(|host| host.id == *seen)
                && complete_round.host_has(*seen) == HostHas::No
        })
        .map(|(_, sighting)| sighting.actor)
        .collect();
    assert_eq!(guest_only.len(), GUEST_ONLY_GROUPS.len());
    let mut watch = GuestOnlyWatch::new();
    let first_round_ms = 10_000;
    assert!(
        watch
            .expired_after(first_round_ms, &guest_only, FIGHT_GUEST_ONLY_ENEMY_GRACE_MS)
            .is_empty()
    );
    let mut removed = watch.expired_after(
        first_round_ms + FIGHT_GUEST_ONLY_ENEMY_GRACE_MS,
        &guest_only,
        FIGHT_GUEST_ONLY_ENEMY_GRACE_MS,
    );
    removed.sort_unstable();
    assert_eq!(removed, vec![1, 2, 3, 4]);
}
