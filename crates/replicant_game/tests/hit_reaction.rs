use replicant_game::enemies::{HitReaction, player_reaction_kind};
use replicant_game::offsets::{
    ATTACK_RECORD_FLAGS_OFFSET, ATTACK_RECORD_HIT_DIRECTION_OFFSET,
    ATTACK_RECORD_PUSH_STRENGTH_OFFSET,
};

use crate::fake_memory::FakeMemory;

const RECORD: u64 = 0x2_9000_0000;
const WIDE_SHOT_FLAGS_OF_THE_RUN: u32 = 0x80_0103;
const DIRECT_HIT_RECORD_FLAGS: u32 = 0x200_1012;

fn wide_shot() -> HitReaction {
    HitReaction {
        flags: WIDE_SHOT_FLAGS_OF_THE_RUN,
        push: 0.0,
        direction: [0.0; 3],
    }
}

#[test]
fn the_guest_s_direct_hit_asks_its_game_for_no_reaction() {
    assert_eq!(player_reaction_kind(DIRECT_HIT_RECORD_FLAGS), 0);
}

#[test]
fn the_run_s_wide_shot_asks_for_reaction_kind_5() {
    assert_eq!(wide_shot().kind(), 5);
}

#[test]
fn the_direct_hit_takes_the_host_record_s_reaction_and_keeps_its_forced_damage() {
    let flags = wide_shot().flags_for_direct_hit(DIRECT_HIT_RECORD_FLAGS);
    assert_eq!(flags, 0x1112);
    assert_eq!(player_reaction_kind(flags), 5);
    let medium = HitReaction {
        flags: 0x4,
        ..wide_shot()
    };
    assert_eq!(
        player_reaction_kind(medium.flags_for_direct_hit(DIRECT_HIT_RECORD_FLAGS)),
        2
    );
    let none = HitReaction {
        flags: 0x200_0000,
        ..wide_shot()
    };
    assert_eq!(
        player_reaction_kind(none.flags_for_direct_hit(DIRECT_HIT_RECORD_FLAGS)),
        0
    );
}

#[test]
fn the_reaction_kinds_follow_the_player_hit_handler_s_order() {
    assert_eq!(player_reaction_kind(0), 1);
    assert_eq!(player_reaction_kind(0x4), 2);
    assert_eq!(player_reaction_kind(0x8 | 0x4), 3);
    assert_eq!(player_reaction_kind(0x100 | 0x8), 5);
    assert_eq!(player_reaction_kind(0x80 | 0x100), 4);
}

#[test]
fn a_record_s_reaction_is_read_with_its_push_and_direction() {
    let mut memory = FakeMemory::new();
    memory.put(
        RECORD + ATTACK_RECORD_FLAGS_OFFSET,
        &WIDE_SHOT_FLAGS_OF_THE_RUN.to_le_bytes(),
    );
    memory.put(
        RECORD + ATTACK_RECORD_PUSH_STRENGTH_OFFSET,
        &2.5_f32.to_le_bytes(),
    );
    for (axis, value) in (0_u64..).zip([0.6_f32, 0.0, -0.8]) {
        memory.put(
            RECORD + ATTACK_RECORD_HIT_DIRECTION_OFFSET + axis * 4,
            &value.to_le_bytes(),
        );
    }
    assert_eq!(
        HitReaction::read(&memory, RECORD),
        Some(HitReaction {
            flags: WIDE_SHOT_FLAGS_OF_THE_RUN,
            push: 2.5,
            direction: [0.6, 0.0, -0.8],
        })
    );
}
