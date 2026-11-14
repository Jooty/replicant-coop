use replicant_game::attack_gauge::{
    ATTACK_GAUGE_BOSS_HANDLE_OFFSET, ATTACK_GAUGE_DUMMY_HANDLE_OFFSET,
    ATTACK_GAUGE_FIRST_NODE_OFFSET, ATTACK_GAUGE_MANAGER_RVA, ATTACK_GAUGE_MAP_POINTS_OFFSET,
    ATTACK_GAUGE_MODE_OFFSET, ATTACK_GAUGE_NODE_HP_OFFSET, ATTACK_GAUGE_NODE_MAX_HP_OFFSET,
    ATTACK_GAUGE_NODE_NEXT_OFFSET, ATTACK_GAUGE_OPTIONS_OFFSET, ATTACK_GAUGE_PARTNER_HANDLE_OFFSET,
    ATTACK_GAUGE_REMAINING_SECONDS_OFFSET, ATTACK_GAUGE_SWITCH_CHECK_INTERVAL_MS,
    ATTACK_GAUGE_TOTAL_SECONDS_OFFSET, GaugeHitDrop, GaugeHitFacts, GaugeHitVerdict,
    GaugeOffSwitch, GuestGaugeFacts, GuestGaugeHits, GuestGaugeStep, GuestGaugeWait,
    HOST_GAUGE_RESEND_INTERVAL_MS, HostGaugeHits, HostGaugeView, HostGaugeWatch, LocalGaugeView,
    attack_gauge_boot_text, attack_gauge_off_file, gauge_hit_applied_text, gauge_open_text,
    guest_gauge_step, guest_node_hp, guest_node_hp_after_own_hit, guest_remaining_floor,
    measured_gauge_damage, node_hp_after_damage, own_gauge_hit_text, read_gauge, write_node_hp,
};
use replicant_game::offsets::{HANDLE_POOL_GENERATIONS_RVA, HANDLE_POOL_POINTERS_RVA};
use replicant_proto::{
    AttackGaugeHit, AttackGaugeNode, AttackGaugeOpen, EnemyId, MAX_ATTACK_GAUGE_NODES,
};

use crate::fake_memory::{FAKE_MODULE_BASE, FakeMemory};

const BOSS: EnemyId = EnemyId {
    group: 2,
    ordinal: 0,
};
const OTHER_BOSS: EnemyId = EnemyId {
    group: 3,
    ordinal: 0,
};
const NODE_HEAP: u64 = 0x3_0000_0000;
const NODE_STRIDE: u64 = 0x100;

fn node(hp: i32) -> AttackGaugeNode {
    AttackGaugeNode {
        bone: 0x28,
        size: 0.6,
        offset: [0.0, 0.5, 0.0, 0.0],
        kind: 0,
        extra: 1,
        hp,
        max_hp: 40,
    }
}

fn open_gauge(remaining: f32, hps: &[i32]) -> AttackGaugeOpen {
    AttackGaugeOpen {
        boss: BOSS,
        partner: None,
        total_seconds: 8.0,
        remaining_seconds: remaining,
        map_points: [-1, -1, -1],
        options: [1, 0],
        nodes: hps.iter().map(|hp| node(*hp)).collect(),
    }
}

fn hit(seq: u32, damage: i32) -> AttackGaugeHit {
    AttackGaugeHit {
        phase: 23,
        serial: 1,
        node: 0,
        damage,
        seq,
    }
}

fn facts(serial: Option<u32>, node_hp: Option<(i32, i32)>) -> GaugeHitFacts {
    GaugeHitFacts {
        local_phase: 23,
        serial,
        node_hp,
        floor_percent: 0,
        switched_off: false,
    }
}

#[test]
fn the_host_sends_an_opening_at_once_with_a_new_number() {
    let mut watch = HostGaugeWatch::new();
    let sent = watch.observe(0, 23, Some(open_gauge(8.0, &[40]))).unwrap();
    assert_eq!(sent.serial, 1);
    assert_eq!(sent.open, Some(open_gauge(8.0, &[40])));
    assert_eq!(watch.serial(), Some(1));
}

#[test]
fn the_host_resends_only_on_a_node_change_or_after_the_interval() {
    let mut watch = HostGaugeWatch::new();
    watch.observe(0, 23, Some(open_gauge(8.0, &[40])));
    assert_eq!(watch.observe(10, 23, Some(open_gauge(7.9, &[40]))), None);
    let changed = watch.observe(20, 23, Some(open_gauge(7.8, &[34]))).unwrap();
    assert_eq!(changed.serial, 1);
    let due = HOST_GAUGE_RESEND_INTERVAL_MS + 20;
    let resent = watch
        .observe(due, 23, Some(open_gauge(7.5, &[34])))
        .unwrap();
    assert_eq!(resent.serial, 1);
}

#[test]
fn the_host_sends_one_closing_with_the_same_number() {
    let mut watch = HostGaugeWatch::new();
    watch.observe(0, 23, Some(open_gauge(8.0, &[40])));
    let closed = watch.observe(10, 23, None).unwrap();
    assert_eq!((closed.serial, closed.open), (1, None));
    assert_eq!(watch.observe(20, 23, None), None);
    assert_eq!(watch.serial(), None);
}

#[test]
fn a_clock_that_went_up_or_another_boss_is_a_new_opening() {
    let mut watch = HostGaugeWatch::new();
    watch.observe(0, 23, Some(open_gauge(3.0, &[40])));
    let reopened = watch.observe(10, 23, Some(open_gauge(8.0, &[40]))).unwrap();
    assert_eq!(reopened.serial, 2);
    let mut other = open_gauge(7.0, &[40]);
    other.boss = OTHER_BOSS;
    assert_eq!(watch.observe(20, 23, Some(other)).unwrap().serial, 3);
}

#[test]
fn a_guest_hit_takes_its_damage_off_the_host_node() {
    let mut hits = HostGaugeHits::new();
    let verdict = hits.judge(&facts(Some(1), Some((40, 40))), &hit(1, 6));
    assert_eq!(
        verdict,
        GaugeHitVerdict::Apply {
            hp_before: 40,
            hp_after: 34,
            breaks: false
        }
    );
}

#[test]
fn a_guest_hit_that_empties_the_node_breaks_it() {
    let mut hits = HostGaugeHits::new();
    let verdict = hits.judge(&facts(Some(1), Some((4, 40))), &hit(1, 6));
    assert_eq!(
        verdict,
        GaugeHitVerdict::Apply {
            hp_before: 4,
            hp_after: -2,
            breaks: true
        }
    );
}

#[test]
fn the_same_guest_hit_is_applied_once() {
    let mut hits = HostGaugeHits::new();
    hits.judge(&facts(Some(1), Some((40, 40))), &hit(7, 6));
    assert_eq!(
        hits.judge(&facts(Some(1), Some((34, 40))), &hit(7, 6)),
        GaugeHitVerdict::Drop(GaugeHitDrop::Repeated)
    );
}

#[test]
fn a_guest_hit_for_another_phase_opening_node_or_a_closed_gauge_is_dropped() {
    let mut hits = HostGaugeHits::new();
    let mut other_phase = hit(1, 6);
    other_phase.phase = 24;
    let drop = |verdict| match verdict {
        GaugeHitVerdict::Drop(reason) => Some(reason),
        GaugeHitVerdict::Apply { .. } | GaugeHitVerdict::LeaveToGame => None,
    };
    assert_eq!(
        drop(hits.judge(&facts(Some(1), Some((40, 40))), &other_phase)),
        Some(GaugeHitDrop::OtherPhase)
    );
    assert_eq!(
        drop(hits.judge(&facts(None, Some((40, 40))), &hit(2, 6))),
        Some(GaugeHitDrop::Closed)
    );
    assert_eq!(
        drop(hits.judge(&facts(Some(2), Some((40, 40))), &hit(3, 6))),
        Some(GaugeHitDrop::OtherOpening)
    );
    assert_eq!(
        drop(hits.judge(&facts(Some(1), None), &hit(4, 6))),
        Some(GaugeHitDrop::NoSuchNode)
    );
    assert_eq!(
        drop(hits.judge(&facts(Some(1), Some((0, 40))), &hit(5, 6))),
        Some(GaugeHitDrop::NodeAlreadyBroken)
    );
}

#[test]
fn a_dropped_hit_does_not_use_up_its_number() {
    let mut hits = HostGaugeHits::new();
    hits.judge(&facts(None, Some((40, 40))), &hit(9, 6));
    assert!(matches!(
        hits.judge(&facts(Some(1), Some((40, 40))), &hit(9, 6)),
        GaugeHitVerdict::Apply { .. }
    ));
}

#[test]
fn the_hp_floor_percent_keeps_a_node_from_breaking() {
    assert_eq!(node_hp_after_damage(10, 40, 30, 25), 10);
    assert_eq!(node_hp_after_damage(30, 40, 5, 25), 25);
    assert_eq!(node_hp_after_damage(10, 40, 30, 0), -20);
}

#[test]
fn the_guest_numbers_its_hits_and_drops_a_swing_on_the_same_node_again() {
    let mut hits = GuestGaugeHits::new();
    assert_eq!(hits.take(0, 0, 77), Some(1));
    assert_eq!(hits.take(100, 0, 77), None);
    assert_eq!(hits.take(200, 1, 77), Some(2));
    assert_eq!(hits.take(300, 0, 78), Some(3));
    assert_eq!(hits.take(2_500, 0, 77), Some(4));
}

#[test]
fn the_measured_damage_is_what_the_hit_took_from_the_measuring_hp() {
    assert_eq!(measured_gauge_damage(1_000_000, 999_994), Some(6));
    assert_eq!(measured_gauge_damage(1_000_000, 1_000_000), None);
}

fn guest_facts(host: Option<HostGaugeView>, local: Option<LocalGaugeView>) -> GuestGaugeFacts {
    GuestGaugeFacts {
        local_phase: 23,
        host,
        local,
        boss_copy_found: true,
        partner_copy_found: true,
        local_only_for_ms: 0,
        asked_host: false,
        switched_off: false,
    }
}

const HOST_OPEN: HostGaugeView = HostGaugeView {
    phase: 23,
    serial: 4,
    boss: BOSS,
    partner: None,
};

#[test]
fn the_guest_opens_the_host_gauge_on_its_copy_of_the_boss() {
    assert_eq!(
        guest_gauge_step(&guest_facts(Some(HOST_OPEN), None)),
        GuestGaugeStep::Open
    );
    let mut no_copy = guest_facts(Some(HOST_OPEN), None);
    no_copy.boss_copy_found = false;
    assert_eq!(
        guest_gauge_step(&no_copy),
        GuestGaugeStep::Wait(GuestGaugeWait::BossCopyMissing)
    );
    let mut pair = guest_facts(
        Some(HostGaugeView {
            partner: Some(OTHER_BOSS),
            ..HOST_OPEN
        }),
        None,
    );
    pair.partner_copy_found = false;
    assert_eq!(
        guest_gauge_step(&pair),
        GuestGaugeStep::Wait(GuestGaugeWait::PartnerCopyMissing)
    );
}

#[test]
fn the_guest_follows_the_same_opening_and_takes_over_its_own_on_the_same_boss() {
    let mirrored = LocalGaugeView {
        boss: Some(BOSS),
        mirrored_serial: Some(4),
    };
    assert_eq!(
        guest_gauge_step(&guest_facts(Some(HOST_OPEN), Some(mirrored))),
        GuestGaugeStep::Mirror
    );
    let own = LocalGaugeView {
        boss: Some(BOSS),
        mirrored_serial: None,
    };
    assert_eq!(
        guest_gauge_step(&guest_facts(Some(HOST_OPEN), Some(own))),
        GuestGaugeStep::Mirror
    );
}

#[test]
fn the_guest_closes_a_gauge_for_another_opening_or_boss() {
    let stale = LocalGaugeView {
        boss: Some(BOSS),
        mirrored_serial: Some(3),
    };
    assert_eq!(
        guest_gauge_step(&guest_facts(Some(HOST_OPEN), Some(stale))),
        GuestGaugeStep::CloseOtherOpening
    );
    let other_boss = LocalGaugeView {
        boss: Some(OTHER_BOSS),
        mirrored_serial: None,
    };
    assert_eq!(
        guest_gauge_step(&guest_facts(Some(HOST_OPEN), Some(other_boss))),
        GuestGaugeStep::CloseOtherOpening
    );
}

#[test]
fn the_guest_closes_when_the_host_closed_and_an_unshared_guest_only_gauge_after_a_grace() {
    let mirrored = LocalGaugeView {
        boss: Some(BOSS),
        mirrored_serial: Some(4),
    };
    assert_eq!(
        guest_gauge_step(&guest_facts(None, Some(mirrored))),
        GuestGaugeStep::CloseHostClosed
    );
    let own = LocalGaugeView {
        boss: None,
        mirrored_serial: None,
    };
    assert_eq!(
        guest_gauge_step(&guest_facts(None, Some(own))),
        GuestGaugeStep::Wait(GuestGaugeWait::GuestOnlyGrace)
    );
    let mut late = guest_facts(None, Some(own));
    late.local_only_for_ms = 1_500;
    assert_eq!(guest_gauge_step(&late), GuestGaugeStep::CloseHostHasNone);
    assert_eq!(
        guest_gauge_step(&guest_facts(None, None)),
        GuestGaugeStep::Nothing
    );
}

#[test]
fn a_host_gauge_in_another_phase_counts_as_none() {
    let other_phase = HostGaugeView {
        phase: 24,
        ..HOST_OPEN
    };
    assert_eq!(
        guest_gauge_step(&guest_facts(Some(other_phase), None)),
        GuestGaugeStep::Nothing
    );
}

#[test]
fn the_guest_never_breaks_a_node_itself() {
    assert_eq!(guest_node_hp(34), 34);
    assert_eq!(guest_node_hp(-2), 0);
    assert_eq!(guest_node_hp_after_own_hit(5, 6), 1);
    assert_eq!(guest_node_hp_after_own_hit(40, 6), 34);
    assert_eq!(guest_node_hp_after_own_hit(0, 6), 0);
}

#[test]
fn the_guest_clock_never_runs_out_on_its_own() {
    assert_eq!(guest_remaining_floor(0.1), Some(0.5));
    assert_eq!(guest_remaining_floor(3.0), None);
}

fn put_handle(memory: &mut FakeMemory, at: u64, slot: u32, actor: u64, generation: u32) {
    memory.put(at, &slot.to_le_bytes());
    memory.put(at + 4, &generation.to_le_bytes());
    memory.put_at_rva(HANDLE_POOL_POINTERS_RVA + slot * 8, &actor.to_le_bytes());
    memory.put_at_rva(
        HANDLE_POOL_GENERATIONS_RVA + slot * 4,
        &generation.to_le_bytes(),
    );
}

fn put_gauge(memory: &mut FakeMemory, mode: i32, node_count: usize, boss_generation: u32) {
    let manager = FAKE_MODULE_BASE + u64::from(ATTACK_GAUGE_MANAGER_RVA);
    memory.put(manager + ATTACK_GAUGE_MODE_OFFSET, &mode.to_le_bytes());
    put_handle(
        memory,
        manager + ATTACK_GAUGE_BOSS_HANDLE_OFFSET,
        10,
        0x5000,
        boss_generation,
    );
    put_handle(
        memory,
        manager + ATTACK_GAUGE_PARTNER_HANDLE_OFFSET,
        11,
        0x6000,
        3,
    );
    put_handle(
        memory,
        manager + ATTACK_GAUGE_DUMMY_HANDLE_OFFSET,
        12,
        0x7000,
        5,
    );
    memory.put(
        manager + ATTACK_GAUGE_TOTAL_SECONDS_OFFSET,
        &8.0_f32.to_le_bytes(),
    );
    memory.put(
        manager + ATTACK_GAUGE_REMAINING_SECONDS_OFFSET,
        &6.5_f32.to_le_bytes(),
    );
    for index in 0..3 {
        memory.put(
            manager + ATTACK_GAUGE_MAP_POINTS_OFFSET + index * 4,
            &(-1_i32).to_le_bytes(),
        );
    }
    memory.put(manager + ATTACK_GAUGE_OPTIONS_OFFSET, &[1, 0]);
    let first = if node_count == 0 { 0 } else { NODE_HEAP };
    memory.put(
        manager + ATTACK_GAUGE_FIRST_NODE_OFFSET,
        &first.to_le_bytes(),
    );
    for index in 0..node_count as u64 {
        let address = NODE_HEAP + index * NODE_STRIDE;
        memory.put(address, &[0; 0x58]);
        memory.put(
            address + ATTACK_GAUGE_NODE_HP_OFFSET,
            &(30_i32 + index as i32).to_le_bytes(),
        );
        memory.put(
            address + ATTACK_GAUGE_NODE_MAX_HP_OFFSET,
            &40_i32.to_le_bytes(),
        );
        let next = if index + 1 == node_count as u64 {
            0
        } else {
            address + NODE_STRIDE
        };
        memory.put(address + ATTACK_GAUGE_NODE_NEXT_OFFSET, &next.to_le_bytes());
    }
}

#[test]
fn an_open_gauge_reads_its_actors_clock_and_nodes() {
    let mut memory = FakeMemory::new();
    put_gauge(&mut memory, 1, 2, 9);
    let reading = read_gauge(&memory).unwrap();
    assert_eq!(
        (reading.boss, reading.partner, reading.dummy),
        (0x5000, None, 0x7000)
    );
    assert_eq!(
        (reading.total_seconds, reading.remaining_seconds),
        (8.0, 6.5)
    );
    assert_eq!(reading.options, [1, 0]);
    let hps: Vec<i32> = reading.nodes.iter().map(|node| node.node.hp).collect();
    assert_eq!(hps, [30, 31]);
    assert!(!reading.more_nodes);
}

#[test]
fn a_two_actor_gauge_reads_its_second_actor() {
    let mut memory = FakeMemory::new();
    put_gauge(&mut memory, 2, 1, 9);
    assert_eq!(read_gauge(&memory).unwrap().partner, Some(0x6000));
}

#[test]
fn a_gauge_whose_boss_handle_was_cleared_is_closed() {
    let mut memory = FakeMemory::new();
    put_gauge(&mut memory, 1, 1, 0);
    assert_eq!(read_gauge(&memory), None);
    let mut never_opened = FakeMemory::new();
    put_gauge(&mut never_opened, 0, 1, 9);
    assert_eq!(read_gauge(&never_opened), None);
}

#[test]
fn a_list_longer_than_the_wire_carries_is_marked() {
    let mut memory = FakeMemory::new();
    put_gauge(&mut memory, 1, MAX_ATTACK_GAUGE_NODES + 1, 9);
    let reading = read_gauge(&memory).unwrap();
    assert_eq!(reading.nodes.len(), MAX_ATTACK_GAUGE_NODES);
    assert!(reading.more_nodes);
}

#[test]
fn a_node_hp_write_lands_in_the_node() {
    let mut memory = FakeMemory::new();
    put_gauge(&mut memory, 1, 1, 9);
    let reading = read_gauge(&memory).unwrap();
    assert!(write_node_hp(&mut memory, &reading.nodes[0], 12));
    assert_eq!(read_gauge(&memory).unwrap().nodes[0].node.hp, 12);
}

#[test]
fn the_log_lines_name_numbers_nodes_and_hp() {
    assert_eq!(
        gauge_open_text(1, &open_gauge(8.0, &[40])),
        "attack gauge: host opened number 1 on 2:0, 1 nodes (hp 40/40), 8.0 s, sent"
    );
    assert_eq!(
        gauge_hit_applied_text(&hit(3, 6), 10, 4, false),
        "attack gauge: guest hit number 3 on node 0 of number 1 took 6, hp 10 -> 4"
    );
    assert_eq!(
        own_gauge_hit_text(0, 6, 3, 1),
        "attack gauge: your hit on node 0 took 6, sent to the host as number 3 for opening 1"
    );
}

#[test]
fn with_the_switch_off_every_verdict_leaves_it_to_the_game() {
    let mut hits = HostGaugeHits::new();
    let mut off = facts(Some(1), Some((40, 40)));
    off.switched_off = true;
    assert_eq!(hits.judge(&off, &hit(1, 6)), GaugeHitVerdict::LeaveToGame);
    let mirrored = LocalGaugeView {
        boss: Some(BOSS),
        mirrored_serial: Some(4),
    };
    for (host, local) in [
        (Some(HOST_OPEN), None),
        (Some(HOST_OPEN), Some(mirrored)),
        (None, Some(mirrored)),
        (None, None),
    ] {
        let mut facts = guest_facts(host, local);
        facts.switched_off = true;
        assert_eq!(guest_gauge_step(&facts), GuestGaugeStep::LeaveToGame);
    }
}

#[test]
fn the_switch_is_read_when_a_gauge_opens_and_at_most_once_a_second() {
    let mut switch = GaugeOffSwitch::new();
    let checks = std::cell::Cell::new(0);
    let observe = |switch: &mut GaugeOffSwitch, now: u64, open: bool, exists: bool| {
        switch.observe(now, open, || {
            checks.set(checks.get() + 1);
            exists
        })
    };
    assert!(!observe(&mut switch, 0, false, true));
    assert!(observe(&mut switch, 10, true, true));
    assert!(observe(&mut switch, 20, true, false));
    assert!(observe(
        &mut switch,
        10 + ATTACK_GAUGE_SWITCH_CHECK_INTERVAL_MS,
        true,
        false
    ));
    assert!(observe(&mut switch, 2_000, false, false));
    assert!(!observe(&mut switch, 2_100, true, false));
    assert_eq!(checks.get(), 3);
}

#[test]
fn the_switch_turns_off_mid_gauge_and_logs_once_a_phase() {
    let mut switch = GaugeOffSwitch::new();
    switch.observe(0, true, || false);
    assert!(!switch.should_log_off(23));
    assert!(switch.observe(ATTACK_GAUGE_SWITCH_CHECK_INTERVAL_MS, true, || true));
    assert!(switch.should_log_off(23));
    assert!(!switch.should_log_off(23));
    assert!(switch.should_log_off(24));
}

#[test]
fn the_switch_file_sits_beside_the_logs_directory() {
    assert_eq!(
        attack_gauge_off_file(r"Z:\home\me\.local\share\nier-coop\logs\game-host.log"),
        r"Z:\home\me\.local\share\nier-coop\attack-gauge-off"
    );
    assert_eq!(
        attack_gauge_off_file("/data/rc/logs/game-guest.log"),
        "/data/rc/attack-gauge-off"
    );
    assert_eq!(
        attack_gauge_off_file(r"C:\Games\nier-coop.log"),
        r"C:\Games\attack-gauge-off"
    );
    assert_eq!(attack_gauge_off_file("nier-coop.log"), "attack-gauge-off");
    assert_eq!(attack_gauge_boot_text(false), "attack gauge: sync on");
    assert_eq!(
        attack_gauge_boot_text(true),
        "attack gauge: sync off (attack-gauge-off file present)"
    );
}
