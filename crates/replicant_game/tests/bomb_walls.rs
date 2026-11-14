use replicant_game::bombs::{
    ATTACK_DIRECTION_GIVEN_FLAG, BOMB_BLAST_ATTACK_FLAGS, BREAKABLE_BOMB_ONLY_MODE,
    BREAKABLE_BOMB_ONLY_MODE_OFFSET, BREAKABLE_BOX_VTABLE_RVA, BREAKABLE_BREAK_TIMER_OFFSET,
    BREAKABLE_HITS_LEFT_OFFSET, BreakableClass, BreakableState, BrokenWalls, MAX_PENDING_WALLS,
    SECOND_BREAKABLE_BOX_VTABLE_RVA, WALL_GIVE_UP_MS, WALL_MATCH_DISTANCE, WALL_RETRY_INTERVAL_MS,
    bomb_break_record, break_is_shared, break_started, breakable_class, nearest_breakable,
    read_breakables, same_wall,
};
use replicant_game::offsets::{
    ACTOR_TRANSFORM_BLOCK_BYTES, ACTOR_TRANSFORM_BLOCK_OFFSET, ATTACK_RECORD_FLAGS_OFFSET,
    HANDLE_POOL_POINTERS_RVA, HANDLE_POOL_SLOT_COUNT,
};
use replicant_proto::WallBroken;

use crate::fake_memory::{FAKE_MODULE_BASE, FakeMemory};

const WALL_HEAP: u64 = 0x2_5000_0000;
const WALL_STRIDE: u64 = 0x2_0000;
const PHASE: u16 = 12;

fn empty_pool() -> FakeMemory {
    let mut memory = FakeMemory::new();
    let empty_table = vec![0_u8; HANDLE_POOL_SLOT_COUNT as usize * 8];
    memory.put_at_rva(HANDLE_POOL_POINTERS_RVA, &empty_table);
    memory
}

fn put_actor(memory: &mut FakeMemory, slot: u32, vtable_rva: u32, position: [f32; 3]) -> u64 {
    let actor = WALL_HEAP + u64::from(slot) * WALL_STRIDE;
    memory.put_at_rva(HANDLE_POOL_POINTERS_RVA + slot * 8, &actor.to_le_bytes());
    let vtable = FAKE_MODULE_BASE + u64::from(vtable_rva);
    memory.put(actor, &vtable.to_le_bytes());
    memory.put(
        actor + ACTOR_TRANSFORM_BLOCK_OFFSET,
        &[0_u8; ACTOR_TRANSFORM_BLOCK_BYTES],
    );
    for (row, value) in position.into_iter().enumerate() {
        memory.put(
            actor + ACTOR_TRANSFORM_BLOCK_OFFSET + row as u64 * 16 + 12,
            &value.to_le_bytes(),
        );
    }
    actor
}

fn put_state(memory: &mut FakeMemory, actor: u64, timer: f32, hits: i32, mode: i32) {
    memory.put(actor + BREAKABLE_BREAK_TIMER_OFFSET, &timer.to_le_bytes());
    memory.put(actor + BREAKABLE_HITS_LEFT_OFFSET, &hits.to_le_bytes());
    memory.put(actor + BREAKABLE_BOMB_ONLY_MODE_OFFSET, &mode.to_le_bytes());
}

fn wall(layout_id: Option<u16>, position: [f32; 3]) -> WallBroken {
    WallBroken {
        phase: PHASE,
        layout_id,
        position,
    }
}

fn whole(mode: i32) -> BreakableState {
    BreakableState {
        break_timer: -1.0,
        hits_left: 3,
        mode,
    }
}

#[test]
fn both_breakable_box_classes_are_told_apart_from_other_actors() {
    let mut memory = empty_pool();
    let first = put_actor(&mut memory, 10, BREAKABLE_BOX_VTABLE_RVA, [0.0; 3]);
    let second = put_actor(&mut memory, 11, SECOND_BREAKABLE_BOX_VTABLE_RVA, [0.0; 3]);
    let other = put_actor(&mut memory, 12, 0x00B0_0150, [0.0; 3]);
    assert_eq!(
        breakable_class(&memory, first),
        Some(BreakableClass::BreakableBox)
    );
    assert_eq!(
        breakable_class(&memory, second),
        Some(BreakableClass::SecondBreakableBox)
    );
    assert_eq!(breakable_class(&memory, other), None);
    assert_eq!(breakable_class(&memory, 0), None);
}

#[test]
fn a_breakable_state_reads_its_timer_hits_and_mode() {
    let mut memory = empty_pool();
    let actor = put_actor(&mut memory, 10, BREAKABLE_BOX_VTABLE_RVA, [0.0; 3]);
    put_state(&mut memory, actor, -1.0, 3, BREAKABLE_BOMB_ONLY_MODE);
    let state = BreakableState::read(&memory, actor).unwrap();
    assert!(state.is_whole());
    assert!(state.takes_only_bombs());
    assert_eq!(state.hits_left, 3);
}

#[test]
fn a_break_starts_when_the_timer_leaves_its_whole_value() {
    let broken = BreakableState {
        break_timer: 0.45,
        hits_left: -1,
        mode: BREAKABLE_BOMB_ONLY_MODE,
    };
    assert!(break_started(whole(1), broken));
    assert!(!break_started(whole(1), whole(1)));
    assert!(!break_started(broken, broken));
}

#[test]
fn a_bomb_only_wall_or_a_bomb_blast_is_shared_and_a_sword_on_a_crate_is_not() {
    assert!(break_is_shared(whole(BREAKABLE_BOMB_ONLY_MODE), 0));
    assert!(break_is_shared(whole(0), BOMB_BLAST_ATTACK_FLAGS));
    assert!(!break_is_shared(whole(0), 0x0080));
}

#[test]
fn the_break_record_carries_the_blast_flags_and_a_given_direction() {
    let record = bomb_break_record();
    let at = ATTACK_RECORD_FLAGS_OFFSET as usize;
    let flags = u32::from_le_bytes(record[at..at + 4].try_into().unwrap());
    assert_eq!(flags, BOMB_BLAST_ATTACK_FLAGS | ATTACK_DIRECTION_GIVEN_FLAG);
    assert!(
        record[..at].iter().all(|byte| *byte == 0),
        "the direction and handles stay zero"
    );
}

#[test]
fn only_breakables_are_read_and_the_nearest_within_reach_is_found() {
    let mut memory = empty_pool();
    let near = put_actor(
        &mut memory,
        10,
        BREAKABLE_BOX_VTABLE_RVA,
        [-35.0, 0.0, 105.0],
    );
    put_actor(
        &mut memory,
        11,
        SECOND_BREAKABLE_BOX_VTABLE_RVA,
        [-35.0, 0.0, 123.0],
    );
    put_actor(&mut memory, 12, 0x00B0_0150, [-35.0, 0.0, 105.2]);
    let breakables = read_breakables(&memory).unwrap();
    assert_eq!(breakables.len(), 2);
    let found = nearest_breakable(&breakables, [-35.5, 0.0, 105.5], WALL_MATCH_DISTANCE).unwrap();
    assert_eq!(found.actor, near);
    assert!(nearest_breakable(&breakables, [0.0, 0.0, 0.0], WALL_MATCH_DISTANCE).is_none());
}

#[test]
fn walls_are_the_same_by_layout_id_or_else_by_place() {
    assert!(same_wall(
        wall(Some(7), [0.0; 3]),
        wall(Some(7), [9.0, 0.0, 0.0])
    ));
    assert!(!same_wall(wall(Some(7), [0.0; 3]), wall(Some(8), [0.0; 3])));
    assert!(same_wall(
        wall(None, [1.0, 0.0, 0.0]),
        wall(Some(8), [1.2, 0.0, 0.0])
    ));
    let elsewhere = WallBroken {
        phase: PHASE + 1,
        ..wall(Some(7), [0.0; 3])
    };
    assert!(!same_wall(wall(Some(7), [0.0; 3]), elsewhere));
}

#[test]
fn a_partner_break_waits_for_its_zone_then_is_tried_at_once_and_again_after_the_interval() {
    let mut walls = BrokenWalls::default();
    let broken = wall(Some(7), [0.0; 3]);
    assert!(walls.note(broken));
    assert!(walls.note(broken), "a repeat is the same entry");
    assert_eq!(walls.len(), 1);
    assert!(walls.due(Some(PHASE + 1), 1_000).is_empty());
    assert!(walls.due(None, 1_000).is_empty());
    assert_eq!(walls.due(Some(PHASE), 2_000), vec![broken]);
    assert!(walls.due(Some(PHASE), 2_100).is_empty());
    assert_eq!(
        walls.due(Some(PHASE), 2_000 + WALL_RETRY_INTERVAL_MS),
        vec![broken]
    );
    walls.copied(broken);
    assert!(walls.is_empty());
}

#[test]
fn a_partner_break_never_found_is_given_up_only_after_time_in_its_zone() {
    let mut walls = BrokenWalls::default();
    let broken = wall(None, [3.0, 0.0, 3.0]);
    walls.note(broken);
    assert!(
        walls.take_given_up(WALL_GIVE_UP_MS * 10).is_empty(),
        "never in its zone yet"
    );
    walls.due(Some(PHASE), 5_000);
    assert!(walls.take_given_up(5_000 + WALL_GIVE_UP_MS - 1).is_empty());
    assert_eq!(walls.take_given_up(5_000 + WALL_GIVE_UP_MS), vec![broken]);
    assert!(walls.is_empty());
}

#[test]
fn the_pending_list_is_bounded() {
    let mut walls = BrokenWalls::default();
    for id in 0..MAX_PENDING_WALLS as u16 {
        assert!(walls.note(wall(Some(id), [0.0; 3])));
    }
    assert!(!walls.note(wall(Some(999), [0.0; 3])));
}
