use replicant_game::enemies::{
    CountLog, ENEMY_ID_LIST_STALE_MS, ENEMY_MIRROR_STALE_MS, ENEMY_ROUND_CAPACITY, EnemyMirror,
    GUEST_ONLY_ENEMY_GRACE_MS, GuestOnlyWatch, HostHas, split_id_list, split_snapshot,
};
use replicant_proto::{
    EnemyId, EnemySnapshot, EnemyState, MAX_ENEMY_ID_LIST_PARTS, MAX_ENEMY_IDS_PER_LIST_PART,
    Message, encode,
};

const PHASE: u16 = 5;

fn id(ordinal: u16) -> EnemyId {
    EnemyId { group: 3, ordinal }
}

fn state(ordinal: u16) -> EnemyState {
    EnemyState {
        id: id(ordinal),
        position: [f32::from(ordinal), 0.0, 0.0],
        yaw_radians: 0.0,
        motion: None,
        hp: 10,
        max_hp: 10,
    }
}

fn floor_of(count: u16) -> Vec<EnemyState> {
    (0..count).map(state).collect()
}

fn feed(mirror: &mut EnemyMirror, at_ms: u64, parts: &[EnemySnapshot]) {
    for part in parts {
        mirror.receive_snapshot(at_ms, part);
    }
}

fn feed_ids(mirror: &mut EnemyMirror, at_ms: u64, tick: u32, ids: Vec<EnemyId>) {
    for part in split_id_list(tick, PHASE, ids) {
        mirror.receive_id_list(at_ms, &part);
    }
}

#[test]
fn a_round_of_eighty_holds_a_floor_of_fifty() {
    assert_eq!(ENEMY_ROUND_CAPACITY, 80);
    let parts = split_snapshot(1, PHASE, floor_of(50), &[[0.0; 3]]);
    let sent: usize = parts.iter().map(|part| part.enemies.len()).sum();
    assert_eq!(sent, 50);
    assert_eq!(parts.len(), 10);
    for part in &parts {
        assert_eq!(part.total, 50);
        assert_eq!(part.round, 50);
        assert!(encode(&Message::EnemySnapshot(part.clone())).is_ok());
    }
    let full = split_snapshot(1, PHASE, floor_of(80), &[[0.0; 3]]);
    assert_eq!(full.len(), 16);
    assert!(full.iter().all(|part| part.round == 80));
}

#[test]
fn a_floor_over_the_round_announces_both_numbers() {
    let parts = split_snapshot(1, PHASE, floor_of(90), &[[0.0; 3]]);
    assert!(
        parts
            .iter()
            .all(|part| part.total == 90 && part.round == 80)
    );
}

#[test]
fn the_list_is_complete_when_every_part_arrived_and_the_counts_agree() {
    let mut mirror = EnemyMirror::new();
    let parts = split_snapshot(1, PHASE, floor_of(50), &[[0.0; 3]]);
    feed(&mut mirror, 0, &parts[..9]);
    assert!(!mirror.host_view(0).round_covers_host);
    feed(&mut mirror, 0, &parts[9..]);
    assert!(mirror.host_view(0).round_covers_host);
    assert_eq!(mirror.host_view(0).host_has(id(77)), HostHas::No);
    assert!(
        !mirror
            .host_view(ENEMY_MIRROR_STALE_MS + 1)
            .round_covers_host
    );
}

#[test]
fn a_round_that_left_enemies_out_is_not_complete_even_with_every_part() {
    let mut mirror = EnemyMirror::new();
    let parts = split_snapshot(1, PHASE, floor_of(90), &[[0.0; 3]]);
    feed(&mut mirror, 0, &parts);
    assert!(!mirror.host_view(0).round_covers_host);
    assert_eq!(
        mirror.latest_complete_round().map(|round| round.total),
        Some(90)
    );
}

#[test]
fn an_enemy_outside_the_round_but_in_the_hosts_ids_is_kept() {
    let mut mirror = EnemyMirror::new();
    let all: Vec<EnemyId> = (0..90).map(id).collect();
    feed(
        &mut mirror,
        0,
        &split_snapshot(1, PHASE, floor_of(90), &[[0.0; 3]]),
    );
    feed_ids(&mut mirror, 0, 1, all);
    let view = mirror.host_view(10);
    assert_eq!(view.host_has(id(85)), HostHas::Yes);
    assert_eq!(view.host_has(id(500)), HostHas::No);
}

#[test]
fn an_enemy_the_host_does_not_have_is_held_after_the_grace() {
    let mut mirror = EnemyMirror::new();
    feed(
        &mut mirror,
        0,
        &split_snapshot(1, PHASE, floor_of(90), &[[0.0; 3]]),
    );
    feed_ids(&mut mirror, 0, 1, (0..90).map(id).collect());
    let view = mirror.host_view(10);
    let mut watch = GuestOnlyWatch::new();
    let candidate = 0xAB_u64;
    let lacks = |enemy: EnemyId| view.host_has(enemy) == HostHas::No;
    assert!(lacks(id(600)));
    assert!(watch.expired(0, &[candidate]).is_empty());
    assert_eq!(
        watch.expired(GUEST_ONLY_ENEMY_GRACE_MS, &[candidate]),
        vec![candidate]
    );
}

#[test]
fn before_the_first_id_list_nothing_is_removed() {
    let mut mirror = EnemyMirror::new();
    feed(
        &mut mirror,
        0,
        &split_snapshot(1, PHASE, floor_of(90), &[[0.0; 3]]),
    );
    let view = mirror.host_view(10);
    assert!(!view.is_known());
    assert_eq!(view.host_has(id(600)), HostHas::Unknown);
    assert_eq!(view.host_has(id(85)), HostHas::Unknown);
}

#[test]
fn a_stale_id_list_is_unknown_again() {
    let mut mirror = EnemyMirror::new();
    feed_ids(&mut mirror, 0, 1, (0..90).map(id).collect());
    assert_eq!(mirror.host_view(1).host_has(id(5)), HostHas::Yes);
    assert_eq!(
        mirror.host_view(ENEMY_ID_LIST_STALE_MS + 1).host_has(id(5)),
        HostHas::Unknown
    );
}

#[test]
fn an_id_list_missing_a_part_is_not_used() {
    let mut mirror = EnemyMirror::new();
    let parts = split_id_list(1, PHASE, (0..150).map(id).collect());
    assert_eq!(parts.len(), 3);
    mirror.receive_id_list(0, &parts[0]);
    mirror.receive_id_list(0, &parts[2]);
    assert!(!mirror.host_view(1).is_known());
    mirror.receive_id_list(0, &parts[1]);
    assert!(mirror.host_view(1).is_known());
}

#[test]
fn an_enemy_spawned_or_gone_after_the_list_updates_it() {
    let mut mirror = EnemyMirror::new();
    feed_ids(&mut mirror, 0, 1, (0..90).map(id).collect());
    mirror.receive_spawned(
        1,
        &replicant_proto::EnemySpawned {
            phase: PHASE,
            enemy: state(200),
        },
    );
    assert_eq!(mirror.host_view(2).host_has(id(200)), HostHas::Yes);
    mirror.receive_gone(&replicant_proto::EnemyGone {
        phase: PHASE,
        id: id(3),
        reason: replicant_proto::EnemyGoneReason::Killed,
    });
    assert_eq!(mirror.host_view(2).host_has(id(3)), HostHas::No);
}

#[test]
fn an_id_list_splits_into_parts_that_each_fit_and_covers_the_ids() {
    let ids: Vec<EnemyId> = (0..200).map(id).collect();
    let parts = split_id_list(4, PHASE, ids);
    assert_eq!(parts.len(), 4);
    assert!(
        parts
            .iter()
            .all(|part| part.ids.len() <= MAX_ENEMY_IDS_PER_LIST_PART)
    );
    assert!(parts.iter().all(|part| part.parts == 4));
    assert!(
        parts
            .iter()
            .all(|part| encode(&Message::EnemyIdList(part.clone())).is_ok())
    );
    let over = split_id_list(4, PHASE, (0..2000).map(id).collect());
    assert_eq!(over.len(), usize::from(MAX_ENEMY_ID_LIST_PARTS));
    assert_eq!(split_id_list(4, PHASE, Vec::new()).len(), 1);
}

#[test]
fn the_count_line_comes_once_per_change_and_not_when_the_numbers_agree() {
    let mut log = CountLog::new();
    assert!(log.guest_line(0, 40, 40, 40, 40).is_none());
    let line = log.guest_line(5000, 90, 80, 80, 85).expect("differs");
    assert!(line.contains("host has 90") && line.contains("round holds 80"));
    assert!(log.guest_line(5001, 90, 80, 80, 85).is_none());
    assert!(
        log.guest_line(10_000, 40, 40, 40, 40)
            .expect("agree")
            .contains("agree again")
    );
    assert!(log.guest_line(15_000, 40, 40, 40, 40).is_none());
}

#[test]
fn the_host_says_once_when_it_has_more_than_a_round_holds() {
    let mut log = CountLog::new();
    assert!(log.host_line(50, ENEMY_ROUND_CAPACITY).is_none());
    assert!(log.host_line(90, ENEMY_ROUND_CAPACITY).is_some());
    assert!(log.host_line(90, ENEMY_ROUND_CAPACITY).is_none());
    assert!(log.host_line(91, ENEMY_ROUND_CAPACITY).is_some());
    assert!(log.host_line(50, ENEMY_ROUND_CAPACITY).is_none());
}
