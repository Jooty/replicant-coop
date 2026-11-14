use serde::{Deserialize, Serialize};

use crate::wire::WireError;

pub const MAX_PROLOGUE_LEVEL: i32 = 99;
pub const NO_WEAPON_REPORTED: i32 = -1;
pub const MAX_REPORTED_WEAPON_ID: i32 = 63;
pub const MAX_REPORTED_WEAPON_LEVEL: i32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrologueLevel {
    pub level: i32,
    pub experience: i32,
    pub weapon: i32,
    pub weapon_level: i32,
}

impl PrologueLevel {
    pub fn validate(&self) -> Result<(), WireError> {
        if !(0..=MAX_PROLOGUE_LEVEL).contains(&self.level) {
            return Err(WireError::OutOfRange { field: "level" });
        }
        if self.experience < 0 {
            return Err(WireError::OutOfRange {
                field: "experience",
            });
        }
        if !(NO_WEAPON_REPORTED..=MAX_REPORTED_WEAPON_ID).contains(&self.weapon) {
            return Err(WireError::OutOfRange { field: "weapon" });
        }
        if !(0..=MAX_REPORTED_WEAPON_LEVEL).contains(&self.weapon_level) {
            return Err(WireError::OutOfRange {
                field: "weapon_level",
            });
        }
        Ok(())
    }
}
