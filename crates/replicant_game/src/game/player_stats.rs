use crate::game::{GameError, Replicant};
use crate::memory::GameMemory;
use crate::offsets::{
    PLAYER_PARAM_ARRAY_RVA, PLAYER_PARAM_MAX_HEALTH_OFFSET, PLAYER_PARAM_MAX_MAGIC_OFFSET,
};

impl<M: GameMemory> Replicant<M> {
    pub fn max_health(&self) -> Result<i32, GameError> {
        let address = self.module_address(PLAYER_PARAM_ARRAY_RVA + PLAYER_PARAM_MAX_HEALTH_OFFSET);
        Ok(self.memory().read_i32(address)?)
    }

    pub fn max_magic(&self) -> Result<f32, GameError> {
        let address = self.module_address(PLAYER_PARAM_ARRAY_RVA + PLAYER_PARAM_MAX_MAGIC_OFFSET);
        Ok(self.memory().read_f32(address)?)
    }

    /// The experience the game uses: the swapped body's own copy while it is worn.
    pub fn experience(&self) -> Result<i32, GameError> {
        let field = self.level_fields_in_use().experience;
        Ok(self.memory().read_i32(self.entity_field(field))?)
    }

    pub fn set_experience(&mut self, value: i32) -> Result<(), GameError> {
        let address = self.entity_field(self.level_fields_in_use().experience);
        Ok(self.memory_mut().write_i32(address, value)?)
    }
}
