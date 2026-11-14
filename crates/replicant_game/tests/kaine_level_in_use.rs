use replicant_game::Replicant;
use replicant_game::kaine::{
    KAINE_EXPERIENCE_SAVE_OFFSET, KAINE_LEVEL_SAVE_OFFSET, KAINE_PLAYER_FLAG_SAVE_OFFSET,
    LevelFields, level_fields_in_use,
};
use replicant_game::offsets::{ENTITY_EXPERIENCE_OFFSET, ENTITY_LEVEL_OFFSET, ENTITY_RVA};

use crate::fake_memory::FakeMemory;

const FIRST_BODY_LEVEL: i32 = 0;
const FIRST_BODY_EXPERIENCE: i32 = 0;
const HER_LEVEL: i32 = 21;
const HER_EXPERIENCE: i32 = 308_230;

fn save(flag: i32) -> FakeMemory {
    let mut memory = FakeMemory::new();
    let put = |memory: &mut FakeMemory, offset: u32, value: i32| {
        memory.put_at_rva(ENTITY_RVA + offset, &value.to_le_bytes());
    };
    put(&mut memory, KAINE_PLAYER_FLAG_SAVE_OFFSET as u32, flag);
    put(&mut memory, ENTITY_LEVEL_OFFSET, FIRST_BODY_LEVEL);
    put(&mut memory, ENTITY_EXPERIENCE_OFFSET, FIRST_BODY_EXPERIENCE);
    put(&mut memory, KAINE_LEVEL_SAVE_OFFSET as u32, HER_LEVEL);
    put(
        &mut memory,
        KAINE_EXPERIENCE_SAVE_OFFSET as u32,
        HER_EXPERIENCE,
    );
    memory
}

#[test]
fn her_copy_is_in_use_only_while_her_flag_is_set() {
    assert_eq!(
        level_fields_in_use(1),
        LevelFields {
            level: 0xE3C,
            experience: 0xE40
        }
    );
    assert_eq!(
        level_fields_in_use(0),
        LevelFields {
            level: ENTITY_LEVEL_OFFSET,
            experience: ENTITY_EXPERIENCE_OFFSET
        }
    );
}

#[test]
fn the_level_and_experience_read_are_hers_while_she_is_worn() {
    let game = Replicant::new(save(1));
    assert_eq!(game.level(), Ok(HER_LEVEL));
    assert_eq!(game.experience(), Ok(HER_EXPERIENCE));
    assert_eq!(game.first_body_level(), Ok(FIRST_BODY_LEVEL));
}

#[test]
fn the_level_and_experience_read_are_the_first_bodys_otherwise() {
    let game = Replicant::new(save(0));
    assert_eq!(game.level(), Ok(FIRST_BODY_LEVEL));
    assert_eq!(game.experience(), Ok(FIRST_BODY_EXPERIENCE));
}

#[test]
fn writes_go_to_her_copy_while_she_is_worn() {
    let mut game = Replicant::new(save(1));
    game.set_level(0).unwrap();
    game.set_experience(0).unwrap();
    assert_eq!(game.level(), Ok(0));
    assert_eq!(game.experience(), Ok(0));
}
