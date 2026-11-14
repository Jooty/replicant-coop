use replicant_game::enemies::{
    DAMAGE_STATE_NAME, HOST_HIT_STAGGER_CHECK_WINDOW_MS, HostHitStaggerWatch, StaggerVerdict,
    player_reaction_motion, read_player_state_name,
};
use replicant_game::move_log::CURRENT_STATE_POINTER_OFFSET;

use crate::fake_memory::{FAKE_MODULE_BASE, FakeMemory};

const PLAYER: u64 = 0x2_4000_0000;
const STATE: u64 = 0x2_5000_0000;
const DAMAGE_STATE_VFTABLE_RVA: u64 = 0xC8_CFC8;
const ATTACKER: &str = "31:3";

fn watch_after_a_small_reel_hit() -> HostHitStaggerWatch {
    let mut watch = HostHitStaggerWatch::new();
    watch.hit_applied(ATTACKER, 1, 0.0, false, 1_000);
    watch
}

#[test]
fn reaction_kinds_name_the_damage_motion_the_game_starts() {
    assert_eq!(player_reaction_motion(0), "no reaction");
    assert_eq!(player_reaction_motion(1), "damage_reel_s");
    assert_eq!(player_reaction_motion(2), "damage_reel_m");
    assert_eq!(player_reaction_motion(4), "damage_blow");
    assert_eq!(player_reaction_motion(5), "damage_hblow");
}

#[test]
fn a_host_hit_with_no_push_that_reels_the_body_is_seen_as_a_stagger() {
    let mut watch = watch_after_a_small_reel_hit();
    assert_eq!(watch.observe("StateActionMove", 1_016), None);
    let verdict = watch.observe(DAMAGE_STATE_NAME, 1_033);
    assert!(
        matches!(&verdict, Some(StaggerVerdict::Entered { check, after_ms: 33 }) if check.kind == 1 && check.push == 0.0),
        "{verdict:?}"
    );
    assert_eq!(watch.observe(DAMAGE_STATE_NAME, 1_050), None);
}

#[test]
fn a_body_that_never_reels_is_reported_once_the_window_passes() {
    let mut watch = watch_after_a_small_reel_hit();
    assert_eq!(watch.observe("StateActionAttack", 1_100), None);
    let verdict = watch.observe(
        "StateActionAttack",
        1_000 + HOST_HIT_STAGGER_CHECK_WINDOW_MS,
    );
    assert!(
        matches!(
            &verdict,
            Some(StaggerVerdict::NotEntered {
                last_state: "StateActionAttack",
                ..
            })
        ),
        "{verdict:?}"
    );
}

#[test]
fn no_check_for_a_hit_that_asks_no_reaction_or_lands_on_a_reeling_body() {
    let mut watch = HostHitStaggerWatch::new();
    watch.hit_applied(ATTACKER, 0, 0.0, false, 0);
    watch.hit_applied(ATTACKER, 1, 0.0, true, 0);
    assert_eq!(watch.observe("StateActionMove", 10_000), None);
}

#[test]
fn a_burst_of_hits_keeps_the_first_check() {
    let mut watch = watch_after_a_small_reel_hit();
    watch.hit_applied("31:4", 2, 0.5, false, 1_010);
    let verdict = watch.observe(DAMAGE_STATE_NAME, 1_020);
    assert!(
        matches!(&verdict, Some(StaggerVerdict::Entered { check, .. }) if check.attacker == ATTACKER),
        "{verdict:?}"
    );
}

#[test]
fn the_state_name_is_read_from_the_players_current_state() {
    let mut memory = FakeMemory::new();
    assert_eq!(read_player_state_name(&memory, PLAYER), "unknown");
    memory.put(PLAYER + CURRENT_STATE_POINTER_OFFSET, &STATE.to_le_bytes());
    memory.put(
        STATE,
        &(FAKE_MODULE_BASE + DAMAGE_STATE_VFTABLE_RVA).to_le_bytes(),
    );
    assert_eq!(read_player_state_name(&memory, PLAYER), DAMAGE_STATE_NAME);
}
