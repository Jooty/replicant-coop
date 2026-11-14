use crate::bombs::{
    ATTACK_DIRECTION_GIVEN_FLAG, BOMB_BLAST_ATTACK_FLAGS, BOMB_BLAST_HIT_FLAG,
    BREAKABLE_BOMB_ONLY_MODE, BREAKABLE_BOMB_ONLY_MODE_OFFSET, BREAKABLE_BOX_VTABLE_RVA,
    BREAKABLE_BREAK_TIMER_OFFSET, BREAKABLE_HITS_LEFT_OFFSET, SECOND_BREAKABLE_BOX_VTABLE_RVA,
    SYNTHETIC_ATTACK_RECORD_BYTES,
};
use crate::game::GameError;
use crate::memory::GameMemory;
use crate::offsets::ATTACK_RECORD_FLAGS_OFFSET;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakableClass {
    BreakableBox,
    SecondBreakableBox,
}

/// The fields a breakable keeps for its break: the timer runs from the first break frame to
/// its removal and is negative while it stands whole.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BreakableState {
    pub break_timer: f32,
    pub hits_left: i32,
    pub mode: i32,
}

impl BreakableClass {
    pub fn vtable_rva(self) -> u32 {
        match self {
            Self::BreakableBox => BREAKABLE_BOX_VTABLE_RVA,
            Self::SecondBreakableBox => SECOND_BREAKABLE_BOX_VTABLE_RVA,
        }
    }
}

impl BreakableState {
    pub fn read<M: GameMemory>(memory: &M, actor: u64) -> Result<Self, GameError> {
        Ok(Self {
            break_timer: memory.read_f32(actor + BREAKABLE_BREAK_TIMER_OFFSET)?,
            hits_left: memory.read_i32(actor + BREAKABLE_HITS_LEFT_OFFSET)?,
            mode: memory.read_i32(actor + BREAKABLE_BOMB_ONLY_MODE_OFFSET)?,
        })
    }

    pub fn is_whole(&self) -> bool {
        self.break_timer < 0.0
    }

    pub fn takes_only_bombs(&self) -> bool {
        self.mode == BREAKABLE_BOMB_ONLY_MODE
    }
}

pub fn breakable_class<M: GameMemory>(memory: &M, actor: u64) -> Option<BreakableClass> {
    if actor == 0 {
        return None;
    }
    let vtable = memory.read_u64(actor).ok()?;
    let base = memory.module_base();
    [
        BreakableClass::BreakableBox,
        BreakableClass::SecondBreakableBox,
    ]
    .into_iter()
    .find(|class| vtable == base + u64::from(class.vtable_rva()))
}

pub fn break_started(before: BreakableState, after: BreakableState) -> bool {
    before.is_whole() && !after.is_whole()
}

/// A wall only a bomb breaks is shared, and so is any breakable a bomb blast broke; a crate
/// broken by a sword stays on the game that broke it.
pub fn break_is_shared(state: BreakableState, attack_flags: u32) -> bool {
    state.takes_only_bombs() || attack_flags & BOMB_BLAST_HIT_FLAG != 0
}

/// The attack record the receiver hands its own wall: a bomb blast's flags with a given (zero)
/// push direction, so the wall's own hit code breaks it with its effect and sound.
pub fn bomb_break_record() -> [u8; SYNTHETIC_ATTACK_RECORD_BYTES] {
    let mut record = [0_u8; SYNTHETIC_ATTACK_RECORD_BYTES];
    let flags = BOMB_BLAST_ATTACK_FLAGS | ATTACK_DIRECTION_GIVEN_FLAG;
    let at = ATTACK_RECORD_FLAGS_OFFSET as usize;
    record[at..at + 4].copy_from_slice(&flags.to_le_bytes());
    record
}
