use replicant_game::effects::{
    HitSparkNames, LiveEffects, MAX_TRAILED_WORLD_EFFECTS, WORLD_EFFECT_MOVE_INTERVAL_MS,
    WorldEffectTrail, hit_spark_log_line, read_world_effect, spawn_is_drawn_by_shot_sync,
    spawn_is_sent,
};
use replicant_proto::WORLD_EFFECT_TRANSFORM_FLOATS;

use crate::fake_memory::FakeMemory;

const NAME: u64 = 0x3_0000_0000;
const MATRIX: u64 = 0x3_1000_0000;
const SHOT_SLOT_CREATE_BY_ID_RETURN_RVA: u32 = 0x32_198E;
const STYLED_EFFECT_CREATE_BY_ID_RETURN_RVA: u32 = 0x31_D3FF;

fn at(x: f32) -> [f32; WORLD_EFFECT_TRANSFORM_FLOATS] {
    [1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, 2.0, 0.0, 0.0, 1.0, 3.0]
}

fn put_spawn(
    memory: &mut FakeMemory,
    name: &str,
    transform: &[f32; WORLD_EFFECT_TRANSFORM_FLOATS],
) {
    let mut text = name.as_bytes().to_vec();
    text.push(0);
    memory.put(NAME, &text);
    let bytes: Vec<u8> = transform
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    memory.put(MATRIX, &bytes);
}

#[test]
fn a_dark_magic_effect_spawned_by_name_is_read_with_its_transform() {
    let mut memory = FakeMemory::new();
    put_spawn(&mut memory, "efP_blackExecution_mc01", &at(5.0));
    assert_eq!(
        read_world_effect(&memory, NAME, MATRIX, false).unwrap(),
        Some(("efP_blackExecution_mc01".to_owned(), at(5.0)))
    );
}

#[test]
fn any_other_effect_spawned_by_name_is_not_read() {
    let mut memory = FakeMemory::new();
    put_spawn(&mut memory, "efE_enemyHit01", &at(5.0));
    assert_eq!(
        read_world_effect(&memory, NAME, MATRIX, false).unwrap(),
        None
    );
    assert_eq!(read_world_effect(&memory, 0, MATRIX, false).unwrap(), None);
}

#[test]
fn any_effect_spawned_by_name_is_read_while_the_hit_wants_every_spark() {
    let mut memory = FakeMemory::new();
    put_spawn(&mut memory, "efE_enemyHit01", &at(5.0));
    assert_eq!(
        read_world_effect(&memory, NAME, MATRIX, true).unwrap(),
        Some(("efE_enemyHit01".to_owned(), at(5.0)))
    );
    assert_eq!(read_world_effect(&memory, 0, MATRIX, true).unwrap(), None);
}

#[test]
fn a_shot_slot_effect_is_left_to_the_shot_sync() {
    assert!(spawn_is_drawn_by_shot_sync(
        SHOT_SLOT_CREATE_BY_ID_RETURN_RVA
    ));
}

#[test]
fn a_styled_start_or_hit_effect_of_a_shot_is_still_sent() {
    assert!(!spawn_is_drawn_by_shot_sync(
        STYLED_EFFECT_CREATE_BY_ID_RETURN_RVA
    ));
}

#[test]
fn a_spawn_outside_any_create_by_id_is_still_sent() {
    assert!(!spawn_is_drawn_by_shot_sync(0));
}

#[test]
fn a_spawn_is_sent_when_outermost_or_when_the_local_hit_causes_it() {
    assert!(spawn_is_sent(true, false));
    assert!(spawn_is_sent(false, true));
    assert!(spawn_is_sent(true, true));
    assert!(!spawn_is_sent(false, false));
}

#[test]
fn a_hit_spark_is_logged_once_per_name() {
    let mut names = HitSparkNames::default();
    assert!(names.is_first("efE_enemyHit01"));
    assert!(!names.is_first("efE_enemyHit01"));
    assert!(names.is_first("efE_enemyHit02"));
}

#[test]
fn a_hit_spark_log_names_the_call_that_made_it() {
    assert_eq!(
        hit_spark_log_line("efE_enemyHit01", "PlayEffect"),
        "effects: hit spark efE_enemyHit01 via PlayEffect"
    );
}

#[test]
fn a_moving_world_effect_is_sent_again_once_per_interval_and_forgotten_when_it_ends() {
    let mut trail = WorldEffectTrail::default();
    assert_eq!(trail.track(7, at(0.0), 0), None);
    assert!(
        trail
            .due_moves(WORLD_EFFECT_MOVE_INTERVAL_MS - 1, |_| Some(at(1.0)))
            .is_empty()
    );
    assert_eq!(
        trail.due_moves(WORLD_EFFECT_MOVE_INTERVAL_MS, |_| Some(at(1.0))),
        vec![(7, at(1.0))]
    );
    assert!(
        trail
            .due_moves(3 * WORLD_EFFECT_MOVE_INTERVAL_MS, |_| Some(at(1.0)))
            .is_empty()
    );
    assert!(
        trail
            .due_moves(4 * WORLD_EFFECT_MOVE_INTERVAL_MS, |_| None)
            .is_empty()
    );
    assert!(trail.is_empty());
}

#[test]
fn a_full_world_effect_trail_drops_its_oldest_handle() {
    let mut trail = WorldEffectTrail::default();
    for handle in 0..MAX_TRAILED_WORLD_EFFECTS as u32 {
        assert_eq!(trail.track(handle, at(0.0), 0), None);
    }
    assert_eq!(trail.track(999, at(0.0), 0), Some(0));
    assert_eq!(trail.len(), MAX_TRAILED_WORLD_EFFECTS);
}

#[test]
fn the_puppet_handle_of_a_sender_handle_is_found_without_taking_it() {
    let mut live = LiveEffects::new();
    live.track(0x40, 0x99, 0);
    assert_eq!(live.own_handle_of(0x99), Some(0x40));
    assert_eq!(live.own_handle_of(0x98), None);
    assert_eq!(live.take_by_sender(0x99), Some(0x40));
}
