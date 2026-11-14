use crate::offsets::{ENTITY_EXPERIENCE_OFFSET, ENTITY_LEVEL_OFFSET};

use super::{KAINE_EXPERIENCE_SAVE_OFFSET, KAINE_LEVEL_SAVE_OFFSET, KAINE_PLAYER_FLAG_SET};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelFields {
    pub level: u32,
    pub experience: u32,
}

/// The save fields the game's own level and experience accessors use (`SaveBlock::GetLevel`,
/// `SetLevel`, `AddExperience`): her copy while her flag is set, the first body's otherwise.
pub fn level_fields_in_use(kaine_flag: i32) -> LevelFields {
    if kaine_flag == KAINE_PLAYER_FLAG_SET {
        LevelFields {
            level: KAINE_LEVEL_SAVE_OFFSET as u32,
            experience: KAINE_EXPERIENCE_SAVE_OFFSET as u32,
        }
    } else {
        LevelFields {
            level: ENTITY_LEVEL_OFFSET,
            experience: ENTITY_EXPERIENCE_OFFSET,
        }
    }
}
