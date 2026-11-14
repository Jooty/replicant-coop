use crate::memory::GameMemory;
use crate::offsets::{
    ATTACK_FLAG_LARGE_REACTION, ATTACK_FLAG_MEDIUM_REACTION, ATTACK_FLAG_NO_PLAYER_REACTION,
    ATTACK_FLAG_REACTION_KIND_4, ATTACK_FLAG_REACTION_KIND_5, ATTACK_RECORD_FLAGS_OFFSET,
    ATTACK_RECORD_HIT_DIRECTION_OFFSET, ATTACK_RECORD_PUSH_STRENGTH_OFFSET,
};

const PLAYER_REACTION_FLAGS: u32 = ATTACK_FLAG_MEDIUM_REACTION
    | ATTACK_FLAG_LARGE_REACTION
    | ATTACK_FLAG_REACTION_KIND_4
    | ATTACK_FLAG_REACTION_KIND_5
    | ATTACK_FLAG_NO_PLAYER_REACTION;
const FLOAT_BYTES: u64 = 4;

/// What a hit record tells the struck player's game about how its body reacts: the reaction
/// bits of its flags, its push strength and its hit direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitReaction {
    pub flags: u32,
    pub push: f32,
    pub direction: [f32; 3],
}

impl HitReaction {
    pub fn read<M: GameMemory>(memory: &M, record: u64) -> Option<Self> {
        let flags = memory
            .read_i32(record + ATTACK_RECORD_FLAGS_OFFSET)
            .ok()?
            .cast_unsigned();
        let push = memory
            .read_f32(record + ATTACK_RECORD_PUSH_STRENGTH_OFFSET)
            .ok()?;
        let mut direction = [0.0_f32; 3];
        for (axis, value) in (0_u64..).zip(direction.iter_mut()) {
            *value = memory
                .read_f32(record + ATTACK_RECORD_HIT_DIRECTION_OFFSET + axis * FLOAT_BYTES)
                .ok()?;
        }
        Some(Self {
            flags,
            push: if push.is_finite() { push.max(0.0) } else { 0.0 },
            direction: direction.map(|axis| if axis.is_finite() { axis } else { 0.0 }),
        })
    }

    /// The flags a direct hit's record carries so that the player's game reacts as it would to
    /// this record: the direct hit's own flags, which keep its damage forced, with this
    /// record's reaction bits in place of its own (a direct hit asks for no reaction at all).
    pub fn flags_for_direct_hit(&self, direct_hit_flags: u32) -> u32 {
        (direct_hit_flags & !PLAYER_REACTION_FLAGS) | (self.flags & PLAYER_REACTION_FLAGS)
    }

    pub fn kind(&self) -> u8 {
        player_reaction_kind(self.flags)
    }
}

/// The reaction the player's hit handler picks from a record's flags before guard and parry
/// results: 0 none, 1 small, 2 medium, 3 large, 4 and 5 for their own flags.
pub fn player_reaction_kind(flags: u32) -> u8 {
    if flags & ATTACK_FLAG_NO_PLAYER_REACTION != 0 {
        0
    } else if flags & ATTACK_FLAG_REACTION_KIND_4 != 0 {
        4
    } else if flags & ATTACK_FLAG_REACTION_KIND_5 != 0 {
        5
    } else if flags & ATTACK_FLAG_LARGE_REACTION != 0 {
        3
    } else if flags & ATTACK_FLAG_MEDIUM_REACTION != 0 {
        2
    } else {
        1
    }
}
