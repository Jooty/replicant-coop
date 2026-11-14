use replicant_game::bombs::{
    BOMB_HOLDER_HANDLE_OFFSET, BOMB_VTABLE_RVA, MAX_OWN_BOMB_HITS, MAX_OWN_BOMBS,
    OWN_BOMB_HIT_MEMORY_MS, OwnBombHits, OwnBombs, SightedBomb, read_bombs,
};
use replicant_game::offsets::{
    ACTOR_TRANSFORM_BLOCK_BYTES, ACTOR_TRANSFORM_BLOCK_OFFSET, HANDLE_POOL_GENERATIONS_RVA,
    HANDLE_POOL_POINTERS_RVA, HANDLE_POOL_SLOT_COUNT,
};
use replicant_game::power_models::{POWER_MODEL_KINDS, power_model_is_prop, power_model_pack_name};
use replicant_game::puppet::PuppetPose;
use replicant_game::{Position, power_models};
use replicant_proto::PowerModelKind;

use crate::fake_memory::{FAKE_MODULE_BASE, FakeMemory};

const _: () = assert!(power_models::MAX_PARTNER_POWER_MODELS > power_models::MAX_SIGHTED_HANDS);

const PLAYER: u64 = 0x2_0000_0000;
const PLAYER_SLOT: u32 = 5;
const GENERATION: u32 = 3;
const BOMB_HEAP: u64 = 0x2_6000_0000;
const BOMB_STRIDE: u64 = 0x2_0000;

fn pool_entry(memory: &mut FakeMemory, slot: u32, actor: u64) {
    memory.put_at_rva(HANDLE_POOL_POINTERS_RVA + slot * 8, &actor.to_le_bytes());
    memory.put_at_rva(
        HANDLE_POOL_GENERATIONS_RVA + slot * 4,
        &GENERATION.to_le_bytes(),
    );
}

fn world() -> FakeMemory {
    let mut memory = FakeMemory::new();
    let slots = HANDLE_POOL_SLOT_COUNT as usize;
    memory.put_at_rva(HANDLE_POOL_POINTERS_RVA, &vec![0_u8; slots * 8]);
    memory.put_at_rva(HANDLE_POOL_GENERATIONS_RVA, &vec![0_u8; slots * 4]);
    pool_entry(&mut memory, PLAYER_SLOT, PLAYER);
    memory.put(PLAYER, &0_u64.to_le_bytes());
    memory
}

fn put_bomb(memory: &mut FakeMemory, slot: u32, holder: Option<(u32, u32)>) -> u64 {
    let actor = BOMB_HEAP + u64::from(slot) * BOMB_STRIDE;
    pool_entry(memory, slot, actor);
    let vtable = FAKE_MODULE_BASE + u64::from(BOMB_VTABLE_RVA);
    memory.put(actor, &vtable.to_le_bytes());
    memory.put(
        actor + ACTOR_TRANSFORM_BLOCK_OFFSET,
        &[0_u8; ACTOR_TRANSFORM_BLOCK_BYTES],
    );
    for (row, value) in [-9.9_f32, -0.5, 37.0].into_iter().enumerate() {
        memory.put(
            actor + ACTOR_TRANSFORM_BLOCK_OFFSET + row as u64 * 16 + 12,
            &value.to_le_bytes(),
        );
    }
    let (index, generation) = holder.unwrap_or((PLAYER_SLOT, 0));
    let mut handle = index.to_le_bytes().to_vec();
    handle.extend_from_slice(&generation.to_le_bytes());
    memory.put(actor + BOMB_HOLDER_HANDLE_OFFSET, &handle);
    actor
}

fn sighted(slot: u16, actor: u64, holder: Option<u64>) -> SightedBomb {
    SightedBomb {
        actor,
        slot,
        pose: PuppetPose {
            position: Position {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            yaw: 0.0,
        },
        holder,
    }
}

#[test]
fn bombs_are_read_with_their_place_and_the_actor_holding_them() {
    let mut memory = world();
    let held = put_bomb(&mut memory, 40, Some((PLAYER_SLOT, GENERATION)));
    let thrown = put_bomb(&mut memory, 41, None);
    let bombs = read_bombs(&memory).unwrap();
    assert_eq!(bombs.len(), 2);
    let first = bombs.iter().find(|bomb| bomb.actor == held).unwrap();
    assert_eq!(first.slot, 40);
    assert_eq!(first.holder, Some(PLAYER));
    assert_eq!(first.pose.position.z, 37.0);
    let second = bombs.iter().find(|bomb| bomb.actor == thrown).unwrap();
    assert_eq!(
        second.holder, None,
        "a thrown bomb's holder handle is cleared"
    );
}

#[test]
fn a_bomb_becomes_yours_when_you_hold_it_and_stays_yours_until_it_is_gone() {
    let mut own = OwnBombs::default();
    let lying = sighted(40, 0xA000, None);
    let changes = own.update(&[lying], PLAYER, 0);
    assert!(changes.taken.is_empty() && changes.live.is_empty());
    let held = sighted(40, 0xA000, Some(PLAYER));
    let changes = own.update(&[held], PLAYER, 100);
    assert_eq!(changes.taken, vec![held]);
    assert_eq!(changes.live, vec![held]);
    let changes = own.update(&[lying], PLAYER, 200);
    assert!(changes.taken.is_empty());
    assert_eq!(changes.live, vec![lying], "thrown, it is still shown");
    assert!(own.is_own(0xA000));
    let changes = own.update(&[], PLAYER, 900);
    assert_eq!(changes.gone, vec![(40, 800)]);
    assert!(!own.is_own(0xA000));
    assert!(own.is_empty());
}

#[test]
fn a_bomb_another_actor_holds_is_not_yours() {
    let mut own = OwnBombs::default();
    let changes = own.update(&[sighted(40, 0xA000, Some(0xBEEF))], PLAYER, 0);
    assert!(changes.taken.is_empty());
    assert!(
        own.update(&[sighted(41, 0xB000, Some(PLAYER))], 0, 0)
            .taken
            .is_empty()
    );
}

#[test]
fn a_reused_slot_with_a_new_actor_ends_the_old_bomb() {
    let mut own = OwnBombs::default();
    own.update(&[sighted(40, 0xA000, Some(PLAYER))], PLAYER, 0);
    let changes = own.update(&[sighted(40, 0xC000, None)], PLAYER, 50);
    assert_eq!(changes.gone, vec![(40, 50)]);
    assert!(changes.live.is_empty());
}

#[test]
fn own_bombs_are_bounded_and_all_end_together_when_the_partner_leaves() {
    let mut own = OwnBombs::default();
    let bombs: Vec<SightedBomb> = (0..MAX_OWN_BOMBS as u16 + 2)
        .map(|slot| sighted(slot, 0xA000 + u64::from(slot), Some(PLAYER)))
        .collect();
    own.update(&bombs, PLAYER, 0);
    assert_eq!(own.len(), MAX_OWN_BOMBS);
    assert_eq!(own.take_all(10).len(), MAX_OWN_BOMBS);
    assert!(own.is_empty());
}

#[test]
fn a_blast_hit_id_counts_as_yours_for_a_while_only() {
    let mut hits = OwnBombHits::default();
    hits.note(77, 1_000);
    assert!(hits.is_own(77, 1_000 + OWN_BOMB_HIT_MEMORY_MS - 1));
    assert!(!hits.is_own(78, 1_000));
    assert!(!hits.is_own(77, 1_000 + OWN_BOMB_HIT_MEMORY_MS));
    hits.note(0, 5_000);
    assert!(!hits.is_own(0, 5_000), "hit id 0 is no attack");
}

#[test]
fn the_remembered_blast_hits_are_bounded_by_dropping_the_oldest() {
    let mut hits = OwnBombHits::default();
    for hit_id in 1..=MAX_OWN_BOMB_HITS as u32 + 1 {
        hits.note(hit_id, 0);
    }
    assert!(!hits.is_own(1, 0));
    assert!(hits.is_own(MAX_OWN_BOMB_HITS as u32 + 1, 0));
}

#[test]
fn the_bomb_is_a_prop_kind_with_its_own_pack_and_no_hand_kind_is_a_prop() {
    assert!(power_model_is_prop(PowerModelKind::Bomb010));
    assert_eq!(power_model_pack_name(PowerModelKind::Bomb010), "BOMB010");
    assert!(!POWER_MODEL_KINDS.into_iter().any(power_model_is_prop));
    assert!(!POWER_MODEL_KINDS.contains(&PowerModelKind::Bomb010));
}
