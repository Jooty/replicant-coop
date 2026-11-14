use std::collections::HashSet;

use replicant_game::enemies::{
    EnemyRoster, EnemySighting, FIGHT_SPAWN_FILL_IN_GRACE_MS, FIGHT_SPAWN_FILL_IN_RETRIES,
    FIGHT_SPAWN_FILL_IN_RETRY_GAP_MS, FightSpawnFillIn, HostFightEnemy, WaitingFightSpawn,
    fight_spawn_fill_in_gave_up_text, fight_spawn_fill_in_snap_off_text,
    fight_spawn_filled_in_text, plan_fight_bindings, plan_takeovers,
};
use replicant_proto::EnemyId;

const PHASE: u16 = 3;
const GROUP: u16 = 6;
const WAVE_SIZE: u16 = 10;
const FIRST_COPY_ACTOR: u64 = 1000;
const FIRST_OWN_ACTOR: u64 = 2000;
const WOLF_CLASS: i32 = 40;
const RUN34_WAVE_CLASS: i32 = 0x15;
const RUN34_WAVES_BEFORE: u16 = 3;
const RUN34_HOST_WAVE_30: [(f32, f32); 10] = [
    (-1.1, -11.7),
    (-6.7, -17.3),
    (2.6, -21.0),
    (4.4, -13.6),
    (6.3, -15.4),
    (4.4, -22.9),
    (-1.1, -17.3),
    (6.3, -21.0),
    (2.6, -17.3),
    (4.4, -17.3),
];
const RUN34_HOST_WAVE_40: [(f32, f32); 10] = [
    (2.6, -13.6),
    (0.7, -17.3),
    (-3.0, -19.2),
    (2.6, -21.0),
    (-3.0, -21.0),
    (-4.9, -17.3),
    (-6.7, -15.4),
    (0.7, -15.4),
    (-3.0, -13.6),
    (0.7, -13.6),
];

fn id(ordinal: u16) -> EnemyId {
    EnemyId {
        group: GROUP,
        ordinal,
    }
}

fn sighting(actor: u64, serial: i32, class_id: i32, x: f32) -> EnemySighting {
    EnemySighting {
        actor,
        spawn_serial: serial,
        group: GROUP,
        class_id,
        position: [x, 0.0, 0.0],
        yaw_radians: 0.0,
        motion: None,
        hp: 10,
        max_hp: 10,
    }
}

fn copy_sightings() -> Vec<EnemySighting> {
    (0..WAVE_SIZE)
        .map(|n| {
            sighting(
                FIRST_COPY_ACTOR + u64::from(n),
                i32::from(n),
                WOLF_CLASS,
                f32::from(n) * 5.0,
            )
        })
        .collect()
}

fn own_sightings() -> Vec<EnemySighting> {
    (0..WAVE_SIZE)
        .rev()
        .map(|n| {
            sighting(
                FIRST_OWN_ACTOR + u64::from(n),
                100 + i32::from(n),
                WOLF_CLASS,
                f32::from(n) * 5.0 + 0.2,
            )
        })
        .collect()
}

fn roster_with_copies() -> EnemyRoster {
    let mut roster = EnemyRoster::new();
    roster.observe(PHASE, &[]);
    for sighting in copy_sightings() {
        roster.claim(
            sighting.actor,
            sighting.spawn_serial,
            id(u16::try_from(sighting.actor - FIRST_COPY_ACTOR).unwrap()),
            sighting.hp,
        );
    }
    roster
}

#[test]
fn own_spawns_at_the_copies_positions_take_over_every_copy() {
    let mut roster = roster_with_copies();
    let mut all = copy_sightings();
    all.extend(own_sightings());
    let mut update = roster.observe(PHASE, &all);
    assert_eq!(update.spawned.len(), usize::from(WAVE_SIZE));
    let host_live: Vec<EnemyId> = (0..WAVE_SIZE).map(id).collect();
    let own_spawns: Vec<_> = update
        .seen
        .iter()
        .filter(|(id, _)| update.spawned.contains(id))
        .copied()
        .collect();
    let copies: Vec<_> = update
        .seen
        .iter()
        .filter(|(id, _)| host_live.contains(id))
        .copied()
        .collect();
    let plan = plan_takeovers(&own_spawns, &copies, &host_live, &HashSet::new());
    assert_eq!(plan.len(), usize::from(WAVE_SIZE));
    for (own, copy) in &plan {
        assert!(roster.take_over_copy(*own, *copy));
        let copy_actor = copies.iter().find(|(id, _)| id == copy).unwrap().1.actor;
        update
            .seen
            .retain(|(_, sighting)| sighting.actor != copy_actor);
    }
    update.rename(&plan);
    assert_eq!(update.seen.len(), usize::from(WAVE_SIZE));
    for (seen_id, sighting) in &update.seen {
        assert!(host_live.contains(seen_id));
        assert!(sighting.actor >= FIRST_OWN_ACTOR);
        assert_eq!(
            sighting.actor - FIRST_OWN_ACTOR,
            u64::from(seen_id.ordinal),
            "own actor at the same spot holds the host id"
        );
    }
    let next = roster.observe(PHASE, &own_sightings());
    assert!(next.spawned.is_empty());
    assert_eq!(next.seen.len(), usize::from(WAVE_SIZE));
    assert_eq!(next.gone.len(), usize::from(WAVE_SIZE));
}

#[test]
fn an_own_spawn_whose_id_the_host_has_is_left_alone() {
    let own = [(id(3), sighting(FIRST_OWN_ACTOR, 1, WOLF_CLASS, 0.0))];
    let copies = [(id(0), sighting(FIRST_COPY_ACTOR, 0, WOLF_CLASS, 0.0))];
    assert!(plan_takeovers(&own, &copies, &[id(0), id(3)], &HashSet::new()).is_empty());
}

#[test]
fn a_copy_of_another_kind_is_kept() {
    let own = [(id(10), sighting(FIRST_OWN_ACTOR, 1, WOLF_CLASS, 0.0))];
    let copies = [(id(0), sighting(FIRST_COPY_ACTOR, 0, WOLF_CLASS + 1, 0.0))];
    assert!(plan_takeovers(&own, &copies, &[id(0)], &HashSet::new()).is_empty());
}

#[test]
fn each_copy_is_taken_by_the_nearest_own_spawn_only_once() {
    let own = [
        (id(10), sighting(FIRST_OWN_ACTOR, 1, WOLF_CLASS, 1.0)),
        (id(11), sighting(FIRST_OWN_ACTOR + 1, 2, WOLF_CLASS, 0.5)),
    ];
    let copies = [(id(0), sighting(FIRST_COPY_ACTOR, 0, WOLF_CLASS, 0.0))];
    assert_eq!(
        plan_takeovers(&own, &copies, &[id(0)], &HashSet::new()),
        vec![(id(11), id(0))]
    );
}

#[test]
fn an_id_an_own_actor_already_holds_is_never_taken_over_again() {
    let own = [(id(11), sighting(FIRST_OWN_ACTOR + 1, 2, WOLF_CLASS, 0.0))];
    let copies = [(id(0), sighting(FIRST_OWN_ACTOR, 1, WOLF_CLASS, 0.0))];
    let held = HashSet::from([id(0)]);
    assert!(plan_takeovers(&own, &copies, &[id(0)], &held).is_empty());
}

#[test]
fn a_far_spawn_does_not_pair_with_a_copy() {
    let own = [(id(10), sighting(FIRST_OWN_ACTOR, 1, WOLF_CLASS, 30.0))];
    let copies = [(id(0), sighting(FIRST_COPY_ACTOR, 0, WOLF_CLASS, 0.0))];
    assert!(plan_takeovers(&own, &copies, &[id(0)], &HashSet::new()).is_empty());
}

fn wave_sightings(
    first_actor: u64,
    first_serial: i32,
    positions: &[(f32, f32)],
) -> Vec<EnemySighting> {
    positions
        .iter()
        .zip(0_u16..)
        .map(|(&(x, z), n)| EnemySighting {
            position: [x, 146.8, z],
            ..sighting(
                first_actor + u64::from(n),
                first_serial + i32::from(n),
                RUN34_WAVE_CLASS,
                0.0,
            )
        })
        .collect()
}

fn roster_after_the_first_waves() -> EnemyRoster {
    let mut roster = EnemyRoster::new();
    roster.observe(PHASE, &[]);
    for wave in 0..RUN34_WAVES_BEFORE {
        let first = u64::from(wave) * 100;
        let serial = i32::from(wave) * 100;
        roster.observe(PHASE, &wave_sightings(first, serial, &RUN34_HOST_WAVE_40));
        roster.observe(PHASE, &[]);
    }
    roster.stop_all_waiting();
    roster
}

fn host_wave(ordinals: std::ops::Range<u16>) -> Vec<HostFightEnemy> {
    ordinals
        .map(|ordinal| HostFightEnemy {
            id: id(ordinal),
            max_hp: 10,
            position: host_wave_position(ordinal),
        })
        .collect()
}

fn waiting_here(roster: &EnemyRoster, seen: &[(EnemyId, EnemySighting)]) -> Vec<WaitingFightSpawn> {
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

fn bind_inside_the_window(
    roster: &mut EnemyRoster,
    seen: &[(EnemyId, EnemySighting)],
    host_live: &[HostFightEnemy],
    taken_over: &HashSet<EnemyId>,
) -> Vec<(EnemyId, EnemyId)> {
    let waiting = waiting_here(roster, seen);
    let waiting_ids: HashSet<EnemyId> = waiting.iter().map(|spawn| spawn.id).collect();
    let held_by_others: HashSet<EnemyId> = roster
        .held_ids()
        .into_iter()
        .filter(|held| !waiting_ids.contains(held))
        .collect();
    let plan = plan_fight_bindings(
        &waiting,
        host_live,
        &HashSet::new(),
        &held_by_others,
        &HashSet::new(),
        taken_over,
    );
    assert!(plan.takeovers.is_empty());
    for kept in &plan.kept {
        roster.stop_waiting(*kept);
    }
    let made = roster.reassign_ids(&plan.renames);
    for (_, to) in &made {
        roster.stop_waiting(*to);
    }
    made
}

fn host_wave_position(ordinal: u16) -> [f32; 3] {
    let wave = if ordinal < 40 {
        RUN34_HOST_WAVE_30
    } else {
        RUN34_HOST_WAVE_40
    };
    let (x, z) = wave[usize::from(ordinal % 10)];
    [x, 146.8, z]
}

#[test]
fn run34_plan_c_copies_made_the_guests_own_late_wave_take_the_next_waves_ids() {
    // run34 G 5902-5911: the guest sat in a menu, so after 3 s the host's 6:30..39 were made as
    // plan C copies; its own wave of those enemies came 22 s later and was numbered 6:40..49.
    let mut roster = roster_after_the_first_waves();
    for (n, _) in RUN34_HOST_WAVE_30.iter().enumerate() {
        let ordinal = 30 + u16::try_from(n).unwrap();
        roster.claim(5_000 + u64::from(ordinal), 5_000, id(ordinal), 24);
    }
    let own_wave = wave_sightings(9_000, 9_000, &RUN34_HOST_WAVE_30);
    let update = roster.observe(PHASE, &own_wave);
    assert_eq!(update.spawned, (40..50).map(id).collect::<Vec<_>>());
    for (seen_id, sighting) in &update.seen {
        assert_ne!(
            sighting.position,
            host_wave_position(seen_id.ordinal),
            "own actor {seen_id:?} stands where the host's wave 30 enemy is, not its own id's"
        );
    }
}

#[test]
fn run34_doubled_waves_inside_a_fight_window_give_one_enemy_per_host_id() {
    // Inside the window no plan C copy is made: the guest's own replayed wave numbers itself.
    let mut roster = roster_after_the_first_waves();
    let taken_over = HashSet::new();
    let wave_30 = wave_sightings(9_000, 9_000, &RUN34_HOST_WAVE_30);
    let host = host_wave(30..50);
    let update = roster.observe(PHASE, &wave_30);
    assert!(bind_inside_the_window(&mut roster, &update.seen, &host, &taken_over).is_empty());
    assert_eq!(update.spawned, (30..40).map(id).collect::<Vec<_>>());
    let mut both = wave_30;
    both.extend(wave_sightings(9_100, 9_100, &RUN34_HOST_WAVE_40));
    let update = roster.observe(PHASE, &both);
    assert!(bind_inside_the_window(&mut roster, &update.seen, &host, &taken_over).is_empty());
    let held: Vec<EnemyId> = update.seen.iter().map(|(seen_id, _)| *seen_id).collect();
    assert_eq!(
        held,
        (30..50).map(id).collect::<Vec<_>>(),
        "one actor per host id"
    );
    for (seen_id, sighting) in &update.seen {
        assert_eq!(sighting.position, host_wave_position(seen_id.ordinal));
    }
}

#[test]
fn copies_made_before_the_window_no_longer_shift_the_replayed_waves_ids() {
    let mut roster = roster_after_the_first_waves();
    for ordinal in 30..40 {
        roster.claim(5_000 + u64::from(ordinal), 5_000, id(ordinal), 24);
    }
    roster.observe(PHASE, &[]);
    let wave_30 = wave_sightings(9_000, 9_000, &RUN34_HOST_WAVE_30);
    let mut update = roster.observe(PHASE, &wave_30);
    assert_eq!(update.spawned, (40..50).map(id).collect::<Vec<_>>());
    let made = bind_inside_the_window(
        &mut roster,
        &update.seen,
        &host_wave(30..40),
        &HashSet::new(),
    );
    assert_eq!(made.len(), 10);
    update.rename(&made);
    for (seen_id, sighting) in &update.seen {
        assert!((30..40).contains(&seen_id.ordinal));
        assert_eq!(sighting.position, host_wave_position(seen_id.ordinal));
    }
    let next = roster.observe(PHASE, &wave_30);
    assert!(next.spawned.is_empty());
    assert_eq!(next.seen.len(), 10);
}

fn waiting_at(ordinal: u16, host_ordinal: u16) -> WaitingFightSpawn {
    WaitingFightSpawn {
        id: id(ordinal),
        max_hp: 10,
        position: host_wave_position(host_ordinal),
    }
}

#[test]
fn a_fight_spawn_never_binds_to_an_id_held_here_or_ever_taken_over() {
    let waiting = [waiting_at(40, 30), waiting_at(41, 31), waiting_at(42, 32)];
    let held_by_others: HashSet<EnemyId> = [id(30)].into_iter().collect();
    let taken_over: HashSet<EnemyId> = [id(31)].into_iter().collect();
    let plan = plan_fight_bindings(
        &waiting,
        &host_wave(30..33),
        &HashSet::new(),
        &held_by_others,
        &HashSet::new(),
        &taken_over,
    );
    assert_eq!(plan.renames, vec![(id(42), id(32))]);
    assert!(plan.takeovers.is_empty());
    assert!(plan.retired.is_empty(), "6:40 and 6:41 are no host ids");
}

#[test]
fn two_replayed_waves_landing_in_one_tick_each_get_their_host_ids() {
    // Copies of 6:30..39 made before the window are gone, so the own ids run 40..59 while the
    // arrivals are 30..49: 6:50 binds to 6:40, which 6:40 frees by binding to 6:30 in the same tick.
    let mut roster = roster_after_the_first_waves();
    for ordinal in 30..40 {
        roster.claim(5_000 + u64::from(ordinal), 5_000, id(ordinal), 24);
    }
    roster.observe(PHASE, &[]);
    let mut both = wave_sightings(9_000, 9_000, &RUN34_HOST_WAVE_30);
    both.extend(wave_sightings(9_100, 9_100, &RUN34_HOST_WAVE_40));
    let mut update = roster.observe(PHASE, &both);
    assert_eq!(update.spawned, (40..60).map(id).collect::<Vec<_>>());
    let made = bind_inside_the_window(
        &mut roster,
        &update.seen,
        &host_wave(30..50),
        &HashSet::new(),
    );
    assert_eq!(
        made.len(),
        20,
        "every own spawn bound, none left on a stale id"
    );
    update.rename(&made);
    let ids: Vec<EnemyId> = update.seen.iter().map(|(seen_id, _)| *seen_id).collect();
    assert_eq!(ids, (30..50).map(id).collect::<Vec<_>>());
    for (seen_id, sighting) in &update.seen {
        assert_eq!(sighting.position, host_wave_position(seen_id.ordinal));
    }
}

#[test]
fn a_chain_and_a_swap_of_ids_are_applied_together() {
    let mut roster = EnemyRoster::new();
    roster.observe(PHASE, &[]);
    for (actor, ordinal) in [(1_u64, 60_u16), (2, 61), (3, 70), (4, 71)] {
        roster.claim(actor, 0, id(ordinal), 10);
    }
    let moves = [
        (id(60), id(61)),
        (id(61), id(62)),
        (id(70), id(71)),
        (id(71), id(70)),
    ];
    assert_eq!(roster.reassign_ids(&moves), moves.to_vec());
    let mut held: Vec<EnemyId> = roster.held_ids().into_iter().collect();
    held.sort();
    assert_eq!(held, vec![id(61), id(62), id(70), id(71)]);
}

#[test]
fn a_move_onto_an_id_whose_holder_stays_is_skipped() {
    let mut roster = EnemyRoster::new();
    roster.observe(PHASE, &[]);
    for (actor, ordinal) in [(1_u64, 50_u16), (2, 51), (3, 52)] {
        roster.claim(actor, 0, id(ordinal), 10);
    }
    let moves = [(id(50), id(51)), (id(51), id(52))];
    assert!(
        roster.reassign_ids(&moves).is_empty(),
        "6:52 stays, so 6:51 cannot move and 6:50 cannot follow it"
    );
    assert_eq!(roster.held_ids().len(), 3);
}

const RUN34_MISSED_INDEX: usize = 9;
const RUN34_MISSED_ORDINAL: u16 = 39;
const HOST_WAVE_ARRIVED_MS: u64 = 10_000;

fn wave_30_without_the_missed_one() -> Vec<EnemySighting> {
    let mut wave = wave_sightings(9_000, 9_000, &RUN34_HOST_WAVE_30);
    wave.remove(RUN34_MISSED_INDEX);
    wave
}

fn held(update_seen: &[(EnemyId, EnemySighting)]) -> HashSet<EnemyId> {
    update_seen.iter().map(|(seen_id, _)| *seen_id).collect()
}

#[test]
fn a_host_fight_spawn_no_guest_spawn_binds_is_filled_in_once_after_the_grace() {
    let host_live: Vec<EnemyId> = (30..40).map(id).collect();
    let copyable: HashSet<EnemyId> = host_live.iter().copied().collect();
    let mut roster = roster_after_the_first_waves();
    let update = roster.observe(PHASE, &wave_30_without_the_missed_one());
    let here = held(&update.seen);
    let mut fill_in = FightSpawnFillIn::default();
    let first = fill_in.observe(
        HOST_WAVE_ARRIVED_MS,
        &host_live,
        &here,
        &HashSet::new(),
        &copyable,
    );
    assert_eq!(first.newly_waiting, vec![id(RUN34_MISSED_ORDINAL)]);
    assert!(first.due.is_empty());
    let early = fill_in.observe(
        HOST_WAVE_ARRIVED_MS + FIGHT_SPAWN_FILL_IN_GRACE_MS - 1,
        &host_live,
        &here,
        &HashSet::new(),
        &copyable,
    );
    assert!(early.newly_waiting.is_empty());
    assert!(early.due.is_empty());
    let due = fill_in.observe(
        HOST_WAVE_ARRIVED_MS + FIGHT_SPAWN_FILL_IN_GRACE_MS,
        &host_live,
        &here,
        &HashSet::new(),
        &copyable,
    );
    assert_eq!(
        due.due,
        vec![(id(RUN34_MISSED_ORDINAL), FIGHT_SPAWN_FILL_IN_GRACE_MS)]
    );
    assert_eq!(
        fight_spawn_filled_in_text(id(RUN34_MISSED_ORDINAL), FIGHT_SPAWN_FILL_IN_GRACE_MS),
        "fight spawn filled in 6:39 after 3000"
    );
    fill_in.mark_filled_in(id(RUN34_MISSED_ORDINAL));
    let after = fill_in.observe(
        HOST_WAVE_ARRIVED_MS + 2 * FIGHT_SPAWN_FILL_IN_GRACE_MS,
        &host_live,
        &here,
        &HashSet::new(),
        &copyable,
    );
    assert!(after.due.is_empty(), "filled in once only");
    assert!(after.newly_waiting.is_empty());
}

#[test]
fn a_host_fight_spawn_bound_in_time_killed_or_not_copyable_is_never_filled_in() {
    let host_live: Vec<EnemyId> = (30..33).map(id).collect();
    let mut fill_in = FightSpawnFillIn::default();
    let nobody_here = HashSet::new();
    let copyable: HashSet<EnemyId> = [id(30), id(31)].into_iter().collect();
    fill_in.observe(0, &host_live, &nobody_here, &HashSet::new(), &copyable);
    let bound_in_time: HashSet<EnemyId> = [id(30)].into_iter().collect();
    let killed: HashSet<EnemyId> = [id(31)].into_iter().collect();
    fill_in.observe(
        FIGHT_SPAWN_FILL_IN_GRACE_MS - 1,
        &host_live,
        &bound_in_time,
        &killed,
        &copyable,
    );
    let later = fill_in.observe(
        10 * FIGHT_SPAWN_FILL_IN_GRACE_MS,
        &host_live,
        &bound_in_time,
        &killed,
        &copyable,
    );
    assert!(
        later.due.is_empty(),
        "6:30 bound, 6:31 killed, 6:32 has no creation"
    );
    fill_in.clear();
    let reopened = fill_in.observe(0, &host_live, &nobody_here, &HashSet::new(), &copyable);
    assert_eq!(reopened.newly_waiting, host_live);
}

#[test]
fn the_guests_late_spawn_takes_over_the_filled_in_copy_so_each_host_id_is_one_enemy() {
    // run34-like: the guest's replay made the first nine of the host's wave 30; 6:39 was filled in after
    // 3 s at the host's spot, and then the guest's own late spawn of it appeared there too.
    let mut roster = roster_after_the_first_waves();
    let mut seen = wave_30_without_the_missed_one();
    roster.observe(PHASE, &seen);
    roster.stop_all_waiting();
    let copy_actor = 5_035;
    roster.claim(copy_actor, 5_035, id(RUN34_MISSED_ORDINAL), 24);
    let copy = EnemySighting {
        position: host_wave_position(RUN34_MISSED_ORDINAL),
        ..sighting(copy_actor, 5_035, RUN34_WAVE_CLASS, 0.0)
    };
    seen.push(copy);
    let update = roster.observe(PHASE, &seen);
    assert!(update.spawned.is_empty());
    assert_eq!(held(&update.seen), (30..40).map(id).collect());
    let late = wave_sightings(9_000, 9_000, &RUN34_HOST_WAVE_30)[RUN34_MISSED_INDEX];
    seen.push(late);
    let mut update = roster.observe(PHASE, &seen);
    assert_eq!(update.spawned.len(), 1);
    let own_id = update.spawned[0];
    assert_ne!(own_id, id(RUN34_MISSED_ORDINAL));
    let waiting = waiting_here(&roster, &update.seen);
    assert_eq!(waiting.len(), 1);
    let copies_here: HashSet<EnemyId> = [id(RUN34_MISSED_ORDINAL)].into_iter().collect();
    let held_by_others: HashSet<EnemyId> = (30..RUN34_MISSED_ORDINAL).map(id).collect();
    let plan = plan_fight_bindings(
        &waiting,
        &host_wave(30..40),
        &HashSet::new(),
        &held_by_others,
        &copies_here,
        &HashSet::new(),
    );
    assert_eq!(plan.takeovers, vec![(own_id, id(RUN34_MISSED_ORDINAL))]);
    assert!(plan.renames.is_empty());
    assert!(roster.take_over_copy(own_id, id(RUN34_MISSED_ORDINAL)));
    update
        .seen
        .retain(|(_, seen_sighting)| seen_sighting.actor != copy_actor);
    update.rename(&plan.takeovers);
    seen.retain(|seen_sighting| seen_sighting.actor != copy_actor);
    let after = roster.observe(PHASE, &seen);
    assert!(after.spawned.is_empty());
    let ids: Vec<EnemyId> = after.seen.iter().map(|(seen_id, _)| *seen_id).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "no host id held twice");
    assert_eq!(sorted, (30..40).map(id).collect::<Vec<_>>());
    for (seen_id, seen_sighting) in &after.seen {
        assert_eq!(seen_sighting.position, host_wave_position(seen_id.ordinal));
    }
}

#[test]
fn a_filled_in_copy_an_own_actor_already_took_over_is_never_taken_again() {
    let waiting = [waiting_at(50, 35)];
    let copies_here: HashSet<EnemyId> = [id(35)].into_iter().collect();
    let taken_over: HashSet<EnemyId> = [id(35)].into_iter().collect();
    let plan = plan_fight_bindings(
        &waiting,
        &host_wave(35..36),
        &HashSet::new(),
        &HashSet::new(),
        &copies_here,
        &taken_over,
    );
    assert!(plan.takeovers.is_empty());
    assert!(plan.renames.is_empty());
}

#[test]
fn a_fill_in_the_game_refuses_is_tried_again_a_second_apart_then_given_up_once() {
    let host_live = vec![id(RUN34_MISSED_ORDINAL)];
    let copyable: HashSet<EnemyId> = host_live.iter().copied().collect();
    let nobody_here = HashSet::new();
    let mut fill_in = FightSpawnFillIn::default();
    fill_in.observe(0, &host_live, &nobody_here, &HashSet::new(), &copyable);
    let mut now = FIGHT_SPAWN_FILL_IN_GRACE_MS;
    let mut tries = 0;
    let mut gave_up = Vec::new();
    while now < 20 * FIGHT_SPAWN_FILL_IN_GRACE_MS {
        let step = fill_in.observe(now, &host_live, &nobody_here, &HashSet::new(), &copyable);
        if !step.due.is_empty() {
            assert!(!fill_in.tried(id(RUN34_MISSED_ORDINAL)) || tries > 0);
            tries += 1;
            gave_up.extend(fill_in.copy_failed(id(RUN34_MISSED_ORDINAL), now));
        }
        now += FIGHT_SPAWN_FILL_IN_RETRY_GAP_MS / 2;
    }
    assert_eq!(
        tries,
        FIGHT_SPAWN_FILL_IN_RETRIES + 1,
        "one try plus the retries"
    );
    assert_eq!(
        gave_up,
        vec![FIGHT_SPAWN_FILL_IN_RETRIES + 1],
        "given up once"
    );
    assert_eq!(
        fight_spawn_fill_in_gave_up_text(id(RUN34_MISSED_ORDINAL), 4),
        "fight spawn fill-in gave up on 6:39 after 4 tries: the game would not create the copy"
    );
}

#[test]
fn a_fill_in_made_on_a_retry_is_filled_in_and_never_due_again() {
    let host_live = vec![id(RUN34_MISSED_ORDINAL)];
    let copyable: HashSet<EnemyId> = host_live.iter().copied().collect();
    let nobody_here = HashSet::new();
    let mut fill_in = FightSpawnFillIn::default();
    fill_in.observe(0, &host_live, &nobody_here, &HashSet::new(), &copyable);
    let first = FIGHT_SPAWN_FILL_IN_GRACE_MS;
    let step = fill_in.observe(first, &host_live, &nobody_here, &HashSet::new(), &copyable);
    assert_eq!(step.due.len(), 1);
    assert_eq!(fill_in.copy_failed(id(RUN34_MISSED_ORDINAL), first), None);
    let too_soon = first + FIGHT_SPAWN_FILL_IN_RETRY_GAP_MS - 1;
    let step = fill_in.observe(
        too_soon,
        &host_live,
        &nobody_here,
        &HashSet::new(),
        &copyable,
    );
    assert!(step.due.is_empty(), "retries wait the gap");
    let retry = first + FIGHT_SPAWN_FILL_IN_RETRY_GAP_MS;
    let step = fill_in.observe(retry, &host_live, &nobody_here, &HashSet::new(), &copyable);
    assert_eq!(step.due.len(), 1);
    fill_in.mark_filled_in(id(RUN34_MISSED_ORDINAL));
    assert!(!fill_in.tried(id(RUN34_MISSED_ORDINAL)));
    let later = retry + 10 * FIGHT_SPAWN_FILL_IN_RETRY_GAP_MS;
    let step = fill_in.observe(later, &host_live, &nobody_here, &HashSet::new(), &copyable);
    assert!(step.due.is_empty());
}

#[test]
fn a_fill_in_skipped_because_snap_is_off_is_logged_once_per_window() {
    let mut fill_in = FightSpawnFillIn::default();
    assert!(fill_in.first_skip_with_snap_off());
    assert!(!fill_in.first_skip_with_snap_off());
    fill_in.clear();
    assert!(
        fill_in.first_skip_with_snap_off(),
        "a new window logs again"
    );
    assert_eq!(
        fight_spawn_fill_in_snap_off_text(id(RUN34_MISSED_ORDINAL)),
        "fight spawn fill-in due for 6:39 but snap is off, so no copy is made (logged once per fight window)"
    );
}
