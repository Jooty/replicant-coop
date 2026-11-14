use replicant_game::attack_gauge::{
    BOSS_ATTACK_GAUGE_BREAKS_LEFT_OFFSET, BOSS_ATTACK_GAUGE_STATE_OFFSET, BossGaugeState,
    GuestOpeningHits, HOST_REPLAYS_GUEST_HIT_AT_MOST_MS_BEFORE_ASK,
    HOST_WAITS_FOR_GUEST_OPENING_HIT_MS, OpeningHitMiss, OpeningHitReplay, OpeningHitStep,
    OpeningReplayReport, host_ask_gave_up_text, host_ask_replayed_text, node_break_text,
};
use replicant_game::enemies::pack_hit_kind;
use replicant_proto::{AttackGaugeAsk, AttackGaugeNode, AttackGaugeOpen, EnemyHit, EnemyId};

use crate::fake_memory::FakeMemory;

const FIGHT_SIX_BOSS: EnemyId = EnemyId {
    group: 100,
    ordinal: 0,
};
const A_WAVE_ENEMY: EnemyId = EnemyId {
    group: 101,
    ordinal: 3,
};
const FIGHT_PHASE: u16 = 2;
const SWORD_FLAGS_OF_THE_OPENING_HIT: u32 = 0x1001_4820;
const OPENING_HIT_APPLIED_AT_MS: u64 = 53_645;
const ASK_ARRIVED_AT_MS: u64 = 53_694;
const BOSS_HP_AFTER_THE_OPENING_HIT: i32 = 6846;
const BOSS_ACTOR: u64 = 0x5_0000_0000;

fn opening_hit(id: EnemyId) -> EnemyHit {
    EnemyHit {
        phase: FIGHT_PHASE,
        id,
        hit_id: 191,
        hp_damage: 7,
        second_gauge: 5,
        third_gauge: 4,
        flags: SWORD_FLAGS_OF_THE_OPENING_HIT,
        attack_kind: pack_hit_kind(0, 2),
    }
}

fn first_ask() -> AttackGaugeAsk {
    AttackGaugeAsk {
        phase: FIGHT_PHASE,
        ask: 1,
        open: AttackGaugeOpen {
            boss: FIGHT_SIX_BOSS,
            partner: None,
            total_seconds: 10.0,
            remaining_seconds: 10.0,
            map_points: [0, 0, 0],
            options: [0, 0],
            nodes: vec![AttackGaugeNode {
                bone: 16,
                size: 1.0,
                offset: [0.0; 4],
                kind: 0,
                extra: 2,
                hp: 344,
                max_hp: 400,
            }],
        },
    }
}

#[test]
fn the_guests_opening_hit_is_replayed_once_with_its_sword_flags_and_no_stagger() {
    let mut hits = GuestOpeningHits::new();
    hits.remember(opening_hit(FIGHT_SIX_BOSS), OPENING_HIT_APPLIED_AT_MS);
    let step = hits.step(
        FIGHT_SIX_BOSS,
        ASK_ARRIVED_AT_MS,
        0,
        BOSS_HP_AFTER_THE_OPENING_HIT,
    );
    let OpeningHitStep::Replay(replay) = step else {
        panic!("the ask 49 ms after the opening hit replays it, got {step:?}");
    };
    assert_eq!(replay.age_ms, 49);
    assert_eq!(replay.hit.flags, SWORD_FLAGS_OF_THE_OPENING_HIT);
    assert_eq!(replay.hit.attack_kind, pack_hit_kind(0, 2));
    assert_eq!(replay.hit.hp_damage, 7);
    assert_eq!((replay.hit.second_gauge, replay.hit.third_gauge), (0, 0));
    assert_eq!(
        hits.step(
            FIGHT_SIX_BOSS,
            ASK_ARRIVED_AT_MS + HOST_WAITS_FOR_GUEST_OPENING_HIT_MS,
            HOST_WAITS_FOR_GUEST_OPENING_HIT_MS,
            BOSS_HP_AFTER_THE_OPENING_HIT,
        ),
        OpeningHitStep::GiveUp(OpeningHitMiss::NoGuestHit),
        "a hit is replayed once"
    );
}

#[test]
fn an_ask_that_arrives_before_its_hit_waits_for_it() {
    let mut hits = GuestOpeningHits::new();
    assert_eq!(
        hits.step(FIGHT_SIX_BOSS, ASK_ARRIVED_AT_MS, 0, 6850),
        OpeningHitStep::Wait
    );
    hits.remember(opening_hit(FIGHT_SIX_BOSS), ASK_ARRIVED_AT_MS + 30);
    assert!(matches!(
        hits.step(FIGHT_SIX_BOSS, ASK_ARRIVED_AT_MS + 32, 32, 6846),
        OpeningHitStep::Replay(OpeningHitReplay { age_ms: 2, .. })
    ));
}

#[test]
fn a_hit_on_another_enemy_or_from_long_before_the_ask_is_never_replayed() {
    let mut hits = GuestOpeningHits::new();
    hits.remember(opening_hit(A_WAVE_ENEMY), OPENING_HIT_APPLIED_AT_MS);
    assert_eq!(
        hits.step(FIGHT_SIX_BOSS, ASK_ARRIVED_AT_MS, 0, 6846),
        OpeningHitStep::Wait
    );
    assert_eq!(
        hits.step(
            FIGHT_SIX_BOSS,
            ASK_ARRIVED_AT_MS + HOST_WAITS_FOR_GUEST_OPENING_HIT_MS,
            HOST_WAITS_FOR_GUEST_OPENING_HIT_MS,
            6846
        ),
        OpeningHitStep::GiveUp(OpeningHitMiss::OtherBoss)
    );
    let mut hits = GuestOpeningHits::new();
    let ask_at = OPENING_HIT_APPLIED_AT_MS + HOST_REPLAYS_GUEST_HIT_AT_MOST_MS_BEFORE_ASK + 1;
    hits.remember(opening_hit(FIGHT_SIX_BOSS), OPENING_HIT_APPLIED_AT_MS);
    assert_eq!(
        hits.step(
            FIGHT_SIX_BOSS,
            ask_at + HOST_WAITS_FOR_GUEST_OPENING_HIT_MS,
            HOST_WAITS_FOR_GUEST_OPENING_HIT_MS,
            6846
        ),
        OpeningHitStep::GiveUp(OpeningHitMiss::TooOld)
    );
}

#[test]
fn the_replay_never_takes_the_boss_below_one_hp() {
    let mut hits = GuestOpeningHits::new();
    hits.remember(opening_hit(FIGHT_SIX_BOSS), OPENING_HIT_APPLIED_AT_MS);
    let OpeningHitStep::Replay(replay) = hits.step(FIGHT_SIX_BOSS, ASK_ARRIVED_AT_MS, 0, 5) else {
        panic!("a boss with 5 hp still gets the replay");
    };
    assert_eq!(replay.hit.hp_damage, 4);
    hits.remember(opening_hit(FIGHT_SIX_BOSS), OPENING_HIT_APPLIED_AT_MS);
    assert_eq!(
        hits.step(FIGHT_SIX_BOSS, ASK_ARRIVED_AT_MS, 0, 1),
        OpeningHitStep::GiveUp(OpeningHitMiss::BossNearlyDead)
    );
}

#[test]
fn a_break_on_a_gauge_the_boss_script_did_not_open_is_named_as_refused() {
    let mut memory = FakeMemory::new();
    memory.put(
        BOSS_ACTOR + BOSS_ATTACK_GAUGE_STATE_OFFSET,
        &0_i32.to_le_bytes(),
    );
    memory.put(
        BOSS_ACTOR + BOSS_ATTACK_GAUGE_BREAKS_LEFT_OFFSET,
        &0_i32.to_le_bytes(),
    );
    let before = BossGaugeState::read(&memory, BOSS_ACTOR);
    assert_eq!(
        before,
        Some(BossGaugeState {
            state: 0,
            breaks_left: 0
        })
    );
    assert!(!before.is_some_and(|state| state.takes_a_break()));
    let line = node_break_text(0, before, false, before);
    assert_eq!(
        line,
        "attack gauge: ran the game's node break for node 0 (boss gauge state 0 (none), breaks left 0 before), the gauge now reads open (boss gauge state 0 (none), breaks left 0 after), the boss refused the break: its script did not open this gauge"
    );
}

#[test]
fn a_break_on_the_boss_scripts_own_gauge_reads_open_then_broken() {
    let open = BossGaugeState {
        state: 1,
        breaks_left: 1,
    };
    let broken = BossGaugeState {
        state: 2,
        breaks_left: 0,
    };
    assert!(open.takes_a_break());
    assert_eq!(
        node_break_text(0, Some(open), true, Some(broken)),
        "attack gauge: ran the game's node break for node 0 (boss gauge state 1 (open), breaks left 1 before), the gauge now reads closed (boss gauge state 2 (broken), breaks left 0 after)"
    );
}

#[test]
fn the_replay_line_names_the_hit_the_hp_and_whether_the_hosts_game_opened_its_gauge() {
    let replay = OpeningHitReplay {
        hit: EnemyHit {
            second_gauge: 0,
            third_gauge: 0,
            ..opening_hit(FIGHT_SIX_BOSS)
        },
        age_ms: 49,
    };
    let report = OpeningReplayReport {
        hp_before: 6846,
        hp_after_hit: 6842,
        hp_put_back: true,
        gauge_opened: true,
        boss_state: Some(BossGaugeState {
            state: 1,
            breaks_left: 1,
        }),
        boss_vtable_rva: Some(0xADF730),
    };
    assert_eq!(
        host_ask_replayed_text(&first_ask(), &replay, &report),
        "attack gauge: guest ask 1 on 100:0: replayed the guest's hit id 191 as your player's hit (flags 0x10014820, kind 0, class 2, hp 7, 49 ms old), boss hp 6846 -> 6842 and put back; your game opened its own gauge: yes; boss gauge state 1 (open), breaks left 1; boss vtable 0xadf730"
    );
    assert_eq!(
        host_ask_gave_up_text(&first_ask(), OpeningHitMiss::NoGuestHit),
        "attack gauge: guest ask 1 on 100:0: no hit replayed (no guest hit on a boss arrived), your game opens no gauge for it"
    );
}
