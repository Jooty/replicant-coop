use replicant_game::attack_gauge::{
    ATTACK_GAUGE_TALLY_INTERVAL_MS, GUEST_ASKED_GAUGE_GRACE_MS, GaugeAskFacts, GaugeAskRefusal,
    GaugeAskVerdict, GaugeHitFacts, GaugeHitVerdict, GaugeSwingMiss, GaugeTally, GuestGaugeFacts,
    GuestGaugeHits, GuestGaugeStep, GuestGaugeWait, HOST_REFUSES_GAUGE_ASK_AFTER_CLOSE_MS,
    HostGaugeAsks, HostGaugeHits, HostGaugeView, LocalGaugeView, gauge_tally_text,
    guest_ask_sent_text, guest_gauge_step, guest_node_hp_after_own_hit, host_ask_refused_text,
    measured_gauge_damage,
};
use replicant_proto::{
    AttackGaugeAsk, AttackGaugeHit, AttackGaugeNode, AttackGaugeOpen, EnemyId, Message, decode,
    encode,
};

const FIGHT_SIX_BOSS: EnemyId = EnemyId {
    group: 100,
    ordinal: 0,
};
const FIGHT_PHASE: u16 = 2;

fn fight_six_node(hp: i32) -> AttackGaugeNode {
    AttackGaugeNode {
        bone: 3,
        size: 1.0,
        offset: [0.0; 4],
        kind: 0,
        extra: 0,
        hp,
        max_hp: 400,
    }
}

fn guest_ask(ask: u32) -> AttackGaugeAsk {
    AttackGaugeAsk {
        phase: FIGHT_PHASE,
        ask,
        open: AttackGaugeOpen {
            boss: FIGHT_SIX_BOSS,
            partner: None,
            total_seconds: 10.0,
            remaining_seconds: 10.0,
            map_points: [0, 0, 0],
            options: [0, 0],
            nodes: vec![fight_six_node(400)],
        },
    }
}

fn ask_facts() -> GaugeAskFacts {
    GaugeAskFacts {
        local_phase: FIGHT_PHASE,
        switched_off: false,
        host_gauge_open: false,
        boss_found: true,
        partner_found: false,
        ms_since_host_close: None,
    }
}

fn own_gauge_facts(asked_host: bool, local_only_for_ms: u64) -> GuestGaugeFacts {
    GuestGaugeFacts {
        local_phase: FIGHT_PHASE,
        host: None,
        local: Some(LocalGaugeView {
            boss: Some(FIGHT_SIX_BOSS),
            mirrored_serial: None,
        }),
        boss_copy_found: true,
        partner_copy_found: true,
        local_only_for_ms,
        asked_host,
        switched_off: false,
    }
}

#[test]
fn a_gauge_the_guests_own_game_opened_on_the_shared_boss_is_asked_of_the_host_once() {
    // Run 1113c, 10:13:51: the guest's hits opened its own gauge 36 s before the host had one.
    assert_eq!(
        guest_gauge_step(&own_gauge_facts(false, 0)),
        GuestGaugeStep::AskHost
    );
    assert_eq!(
        guest_gauge_step(&own_gauge_facts(true, 1_500)),
        GuestGaugeStep::Wait(GuestGaugeWait::HostAsked)
    );
    assert_eq!(
        guest_gauge_step(&own_gauge_facts(true, GUEST_ASKED_GAUGE_GRACE_MS)),
        GuestGaugeStep::CloseHostHasNone
    );
}

#[test]
fn the_asked_gauge_is_taken_over_when_the_hosts_opening_arrives() {
    let mut facts = own_gauge_facts(true, 300);
    facts.host = Some(HostGaugeView {
        phase: FIGHT_PHASE,
        serial: 1,
        boss: FIGHT_SIX_BOSS,
        partner: None,
    });
    assert_eq!(guest_gauge_step(&facts), GuestGaugeStep::Mirror);
}

#[test]
fn the_host_replays_the_guests_opening_hit_when_it_has_no_gauge() {
    let mut asks = HostGaugeAsks::new();
    assert_eq!(
        asks.judge(&ask_facts(), &guest_ask(1)),
        GaugeAskVerdict::ReplayOpeningHit
    );
    assert_eq!(
        asks.judge(&ask_facts(), &guest_ask(1)),
        GaugeAskVerdict::Refuse(GaugeAskRefusal::Repeated)
    );
}

#[test]
fn the_host_refuses_an_ask_while_its_own_gauge_is_open_or_just_after_it_closed() {
    let mut asks = HostGaugeAsks::new();
    let open = GaugeAskFacts {
        host_gauge_open: true,
        ..ask_facts()
    };
    assert_eq!(
        asks.judge(&open, &guest_ask(1)),
        GaugeAskVerdict::Refuse(GaugeAskRefusal::HostGaugeOpen)
    );
    let just_closed = GaugeAskFacts {
        ms_since_host_close: Some(HOST_REFUSES_GAUGE_ASK_AFTER_CLOSE_MS - 1),
        ..ask_facts()
    };
    assert_eq!(
        asks.judge(&just_closed, &guest_ask(2)),
        GaugeAskVerdict::Refuse(GaugeAskRefusal::JustClosed)
    );
    let later = GaugeAskFacts {
        ms_since_host_close: Some(HOST_REFUSES_GAUGE_ASK_AFTER_CLOSE_MS),
        ..ask_facts()
    };
    assert_eq!(
        asks.judge(&later, &guest_ask(3)),
        GaugeAskVerdict::ReplayOpeningHit
    );
}

#[test]
fn the_host_refuses_an_ask_for_another_phase_a_missing_boss_or_with_sync_off() {
    let mut asks = HostGaugeAsks::new();
    let mut other_phase = guest_ask(1);
    other_phase.phase = FIGHT_PHASE + 1;
    assert_eq!(
        asks.judge(&ask_facts(), &other_phase),
        GaugeAskVerdict::Refuse(GaugeAskRefusal::OtherPhase)
    );
    let no_boss = GaugeAskFacts {
        boss_found: false,
        ..ask_facts()
    };
    assert_eq!(
        asks.judge(&no_boss, &guest_ask(2)),
        GaugeAskVerdict::Refuse(GaugeAskRefusal::BossMissing)
    );
    let off = GaugeAskFacts {
        switched_off: true,
        ..ask_facts()
    };
    assert_eq!(
        asks.judge(&off, &guest_ask(3)),
        GaugeAskVerdict::Refuse(GaugeAskRefusal::SwitchedOff)
    );
}

#[test]
fn a_guest_swing_while_the_ask_is_out_lowers_its_node_but_never_breaks_it() {
    // Run 1113c, 10:13:52.9: the guest's game broke its own node; held hits keep it at 1 or more.
    let mut hits = GuestGaugeHits::new();
    let damage = measured_gauge_damage(1_000_000, 1_000_000 - 2_420).unwrap();
    assert_eq!(hits.take(0, 0, 47), Some(1));
    assert_eq!(guest_node_hp_after_own_hit(400, damage), 1);
}

#[test]
fn the_guests_held_hit_breaks_the_hosts_node_once_the_opening_exists() {
    let mut host = HostGaugeHits::new();
    let facts = GaugeHitFacts {
        local_phase: FIGHT_PHASE,
        serial: Some(1),
        node_hp: Some((400, 400)),
        floor_percent: 0,
        switched_off: false,
    };
    let held = AttackGaugeHit {
        phase: FIGHT_PHASE,
        serial: 1,
        node: 0,
        damage: 2_420,
        seq: 1,
    };
    assert_eq!(
        host.judge(&facts, &held),
        GaugeHitVerdict::Apply {
            hp_before: 400,
            hp_after: -2_020,
            breaks: true,
        }
    );
}

#[test]
fn the_ask_round_trips_and_is_relayed() {
    let message = Message::AttackGaugeAsk(guest_ask(7));
    assert!(message.is_relayed());
    assert_eq!(message.validate(), Ok(()));
    assert_eq!(decode(&encode(&message).unwrap()), Ok(message));
    let mut empty = guest_ask(8);
    empty.open.nodes.clear();
    assert!(Message::AttackGaugeAsk(empty).validate().is_err());
}

#[test]
fn the_tally_line_comes_at_most_once_a_second_and_names_the_last_miss() {
    let mut tally = GaugeTally::new();
    assert!(tally.line_due(10_000));
    assert!(!tally.line_due(10_000 + ATTACK_GAUGE_TALLY_INTERVAL_MS - 1));
    assert!(tally.line_due(10_000 + ATTACK_GAUGE_TALLY_INTERVAL_MS));
    tally.sent = 3;
    tally.held = 1;
    tally.miss(GaugeSwingMiss::GameTookNothing);
    let line = gauge_tally_text("guest", Some(1), &[fight_six_node(387)], &tally);
    assert_eq!(
        line,
        "attack gauge: guest number 1: nodes hp 387/400; your hits sent 3, held for the host 1; not counted 1 (last: the game's hit function took nothing (menu open, or not your player's hit))"
    );
    tally.reset();
    assert_eq!(tally, GaugeTally::new());
}

#[test]
fn the_ask_lines_name_the_boss_and_the_reason() {
    assert_eq!(
        guest_ask_sent_text(&guest_ask(1)),
        "attack gauge: your game opened a gauge on 100:0 the host does not have, asked the host to open it (ask 1, 1 nodes, hp 400/400, 10.0 of 10.0 s)"
    );
    assert_eq!(
        host_ask_refused_text(&guest_ask(2), GaugeAskRefusal::HostGaugeOpen),
        "attack gauge: guest ask 2 to open a gauge on 100:0 refused (your own gauge is open, the guest follows it)"
    );
}
