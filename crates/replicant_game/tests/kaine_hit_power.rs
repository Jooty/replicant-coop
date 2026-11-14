use replicant_game::dialogue::GAME_STATE_RVA;
use replicant_game::kaine::{
    KAINE_LEVEL_SAVE_OFFSET, KAINE_PLAYER_FLAG_SAVE_OFFSET, ReferenceSource, ReferenceWeapon,
    SwappedBodyPower, SwappedHitTally, SwappedPowerLog, read_minimum_damages, swapped_power_line,
    weapon_factor, weapon_powered_source,
};
use replicant_game::offsets::{
    ACTOR_PLAYER_PARAM_POINTER_OFFSET, ATTACK_PARAM_MINIMUM_DAMAGE_OFFSETS,
    ATTACK_RECORD_PARAM_RECORD_POINTER_OFFSET, ENTITY_LEVEL_OFFSET, ENTITY_RVA, FORCED_WEAPON_ID,
    GAME_STATE_FIXED_PLAYER_LEVEL_OFFSET, PLAYER_PARAM_CACHED_ATTACK_OFFSET,
    SAVE_EQUIPPED_WEAPON_ID_OFFSET, SAVE_FORCED_WEAPON_FLAG_OFFSET, SAVE_WEAPON_LEVELS_OFFSET,
    WEAPON_RECORD_HP_ATTACK_PERCENT_OFFSETS, WEAPON_RECORD_KIND_OFFSET,
    WEAPON_RECORD_SECOND_GAUGE_PERCENT_OFFSETS, WEAPON_RECORD_THIRD_GAUGE_PERCENT_OFFSETS,
    WEAPON_TABLE_LEN, WEAPON_TABLE_RVA,
};

use crate::fake_memory::FakeMemory;

const RECORD_HEAP: u64 = 0x3_1000_0000;
const RECORD_STRIDE: u64 = 0x200;
const PLAYER: u64 = 0x3_2000_0000;
const PLAYER_PARAM: u64 = 0x3_3000_0000;
const ATTACK_RECORD: u64 = 0x3_4000_0000;
const ATTACK_PARAM_RECORD: u64 = 0x3_5000_0000;
const FIRST_BODY_WEAPON: i32 = 13;
const ONE_HANDED: u8 = 0;
const LARGE_SWORD: u8 = 1;
const HOST_REFERENCE: Option<ReferenceWeapon> = Some(ReferenceWeapon { id: 13, level: 0 });
const LEVEL_IN_THE_RUN: i32 = 21;
const BASE_ATTACK_AT_LEVEL_21: i32 = 0xCA;
const FORCED_WEAPON_PERCENTS: ([i32; 4], [i32; 4], [i32; 4]) = ([999; 4], [220; 4], [330; 4]);
const FIRST_BODY_WEAPON_PERCENTS: ([i32; 4], [i32; 4], [i32; 4]) =
    ([179, 179, 217, 266], [103, 103, 106, 109], [30, 30, 30, 30]);

fn put_weapon_of_kind(
    memory: &mut FakeMemory,
    id: i32,
    kind: u8,
    percents: ([i32; 4], [i32; 4], [i32; 4]),
) {
    let record = RECORD_HEAP + id as u64 * RECORD_STRIDE;
    memory.put_at_rva(WEAPON_TABLE_RVA + id as u32 * 8, &record.to_le_bytes());
    memory.put(record + WEAPON_RECORD_KIND_OFFSET, &[kind]);
    let rows = [
        (WEAPON_RECORD_HP_ATTACK_PERCENT_OFFSETS, percents.0),
        (WEAPON_RECORD_SECOND_GAUGE_PERCENT_OFFSETS, percents.1),
        (WEAPON_RECORD_THIRD_GAUGE_PERCENT_OFFSETS, percents.2),
    ];
    for (offsets, values) in rows {
        for (offset, value) in offsets.into_iter().zip(values) {
            memory.put(record + offset, &value.to_le_bytes());
        }
    }
}

fn put_weapon(memory: &mut FakeMemory, id: i32, percents: ([i32; 4], [i32; 4], [i32; 4])) {
    put_weapon_of_kind(memory, id, ONE_HANDED, percents);
}

fn run_1113b_guest() -> FakeMemory {
    let mut memory = FakeMemory::new();
    memory.put_at_rva(
        ENTITY_RVA + KAINE_PLAYER_FLAG_SAVE_OFFSET as u32,
        &1_i32.to_le_bytes(),
    );
    memory.put_at_rva(ENTITY_RVA + ENTITY_LEVEL_OFFSET, &0_i32.to_le_bytes());
    memory.put_at_rva(
        ENTITY_RVA + KAINE_LEVEL_SAVE_OFFSET as u32,
        &LEVEL_IN_THE_RUN.to_le_bytes(),
    );
    memory.put_at_rva(
        ENTITY_RVA + SAVE_EQUIPPED_WEAPON_ID_OFFSET as u32,
        &FIRST_BODY_WEAPON.to_le_bytes(),
    );
    memory.put_at_rva(
        ENTITY_RVA + SAVE_WEAPON_LEVELS_OFFSET as u32,
        &[0_u8; WEAPON_TABLE_LEN as usize],
    );
    put_weapon(&mut memory, FORCED_WEAPON_ID, FORCED_WEAPON_PERCENTS);
    put_weapon(&mut memory, FIRST_BODY_WEAPON, FIRST_BODY_WEAPON_PERCENTS);
    memory.put(
        PLAYER + ACTOR_PLAYER_PARAM_POINTER_OFFSET,
        &PLAYER_PARAM.to_le_bytes(),
    );
    memory.put(
        PLAYER_PARAM + PLAYER_PARAM_CACHED_ATTACK_OFFSET,
        &BASE_ATTACK_AT_LEVEL_21.to_le_bytes(),
    );
    memory
}

fn power_with(memory: &FakeMemory, host: Option<ReferenceWeapon>) -> SwappedBodyPower {
    SwappedBodyPower::read(memory, PLAYER, host).expect("the swapped body is in play")
}

fn power(memory: &FakeMemory) -> SwappedBodyPower {
    SwappedBodyPower::read(memory, PLAYER, HOST_REFERENCE).expect("the swapped body is in play")
}

#[test]
fn the_forced_weapon_swing_of_the_run_is_cut_to_the_first_body_weapon() {
    let power = power(&run_1113b_guest());
    assert_eq!(power.worn.id, FORCED_WEAPON_ID);
    assert_eq!(power.reference.id, FIRST_BODY_WEAPON);
    assert_eq!(power.level_in_use, LEVEL_IN_THE_RUN);
    assert_eq!(power.source, ReferenceSource::Host);
    assert_eq!(power.scaled([2420, 136, 459], None), [434, 64, 42]);
}

#[test]
fn the_attack_in_use_is_the_base_times_the_worn_weapon_and_the_first_body_gets_its_own() {
    let power = power(&run_1113b_guest());
    assert_eq!(power.attack_in_use(), Some(2018));
    assert_eq!(power.reference_attack(), Some(362));
}

#[test]
fn a_scaled_number_keeps_the_attack_minimum() {
    let power = power(&run_1113b_guest());
    assert_eq!(power.scaled([50, 0, 8], Some([50, 0, 8])), [50, 0, 8]);
}

#[test]
fn the_reference_weapon_level_picks_its_percentages() {
    let power = power_with(
        &run_1113b_guest(),
        Some(ReferenceWeapon { id: 13, level: 3 }),
    );
    assert_eq!(power.scaled([999, 220, 330], None), [266, 109, 30]);
}

#[test]
fn a_fixed_player_level_reads_the_hp_percentage_at_level_zero() {
    let mut memory = run_1113b_guest();
    memory.put_at_rva(
        GAME_STATE_RVA + GAME_STATE_FIXED_PLAYER_LEVEL_OFFSET as u32,
        &5_i32.to_le_bytes(),
    );
    let power = power_with(&memory, Some(ReferenceWeapon { id: 13, level: 2 }));
    assert!(power.fixed_level);
    assert_eq!(power.scaled([999, 220, 330], None), [179, 106, 30]);
}

#[test]
fn nothing_is_read_while_the_first_body_is_in_play() {
    let mut memory = run_1113b_guest();
    memory.put_at_rva(
        ENTITY_RVA + KAINE_PLAYER_FLAG_SAVE_OFFSET as u32,
        &0_i32.to_le_bytes(),
    );
    assert_eq!(
        SwappedBodyPower::read(&memory, PLAYER, HOST_REFERENCE),
        None
    );
}

#[test]
fn a_weapon_level_past_the_stored_range_leaves_the_weapon_out() {
    assert!((weapon_factor([999; 4], 4) - 1.0).abs() < f32::EPSILON);
    assert!((weapon_factor([179, 1, 1, 1], -1) - 1.79).abs() < 1e-6);
}

#[test]
fn only_swings_and_spells_are_powered_by_the_weapon() {
    assert!(weapon_powered_source(1));
    assert!(weapon_powered_source(2));
    assert!(!weapon_powered_source(3));
}

#[test]
fn the_minimums_come_from_the_attack_param_record() {
    let mut memory = FakeMemory::new();
    memory.put(
        ATTACK_RECORD + ATTACK_RECORD_PARAM_RECORD_POINTER_OFFSET,
        &ATTACK_PARAM_RECORD.to_le_bytes(),
    );
    for (offset, value) in ATTACK_PARAM_MINIMUM_DAMAGE_OFFSETS
        .into_iter()
        .zip([50_i32, 0, 8])
    {
        memory.put(ATTACK_PARAM_RECORD + offset, &value.to_le_bytes());
    }
    assert_eq!(
        read_minimum_damages(&memory, ATTACK_RECORD),
        Some([50, 0, 8])
    );
}

#[test]
fn the_power_line_is_written_once_a_second_while_hits_land() {
    let mut log = SwappedPowerLog::new();
    assert_eq!(
        log.note_hit(0, 2420, 434),
        Some(SwappedHitTally {
            hits: 1,
            game_hp: 2420,
            sent_hp: 434
        })
    );
    assert_eq!(log.note_hit(300, 2420, 434), None);
    assert_eq!(log.note_hit(700, 2420, 434), None);
    assert_eq!(
        log.note_hit(1000, 2420, 434),
        Some(SwappedHitTally {
            hits: 3,
            game_hp: 7260,
            sent_hp: 1302
        })
    );
}

#[test]
fn the_power_line_names_the_level_and_the_attack_in_use() {
    let power = power(&run_1113b_guest());
    let line = swapped_power_line(
        &power,
        SwappedHitTally {
            hits: 1,
            game_hp: 2420,
            sent_hp: 434,
        },
    );
    assert_eq!(
        line,
        "swapped body power: level 21 in use, base attack 202, fixed level off, worn weapon 14 lv 0 at 999/220/330%, reference weapon 13 lv 0 at 179/103/30% (from the host), attack 2018 in use, 362 on the reference, hits x0.179/x0.468/x0.091; 1 weapon hits since the last line, hp 2420 from the game, 434 sent"
    );
}

#[test]
fn a_reference_that_reads_as_the_forced_weapon_never_gives_a_scale_of_one() {
    let mut memory = run_1113b_guest();
    put_weapon(&mut memory, 4, ([90, 90, 90, 90], [50; 4], [10; 4]));
    let power = power_with(
        &memory,
        Some(ReferenceWeapon {
            id: FORCED_WEAPON_ID,
            level: 0,
        }),
    );
    assert_eq!(power.source, ReferenceSource::Fallback);
    assert_ne!(power.reference.id, FORCED_WEAPON_ID);
    for scale in power.scale() {
        assert!(scale < 0.5, "{scale}");
    }
}

#[test]
fn without_the_hosts_weapon_the_weakest_one_handed_sword_of_the_table_is_used() {
    let mut memory = run_1113b_guest();
    put_weapon(&mut memory, 4, ([90, 90, 90, 90], [50; 4], [10; 4]));
    put_weapon(&mut memory, 2, ([150; 4], [80; 4], [40; 4]));
    put_weapon_of_kind(&mut memory, 6, LARGE_SWORD, ([10; 4], [10; 4], [10; 4]));
    let power = power_with(&memory, None);
    assert_eq!(power.source, ReferenceSource::Fallback);
    assert_eq!(power.reference.id, 4);
    assert_eq!(power.scaled([999, 220, 330], None), [90, 50, 10]);
}

#[test]
fn the_fallback_reads_the_hosts_weapon_level_when_it_is_known() {
    let mut memory = run_1113b_guest();
    put_weapon(&mut memory, 4, ([90, 90, 120, 130], [50; 4], [10; 4]));
    let power = power_with(
        &memory,
        Some(ReferenceWeapon {
            id: FORCED_WEAPON_ID,
            level: 2,
        }),
    );
    assert_eq!(power.reference.level, 2);
    assert_eq!(power.scaled([999, 220, 330], None), [120, 50, 10]);
}

#[test]
fn a_host_weapon_the_table_does_not_know_falls_back() {
    let power = power_with(
        &run_1113b_guest(),
        Some(ReferenceWeapon { id: 40, level: 0 }),
    );
    assert_eq!(power.source, ReferenceSource::Fallback);
    assert_eq!(power.reference.id, FIRST_BODY_WEAPON);
}

#[test]
fn the_power_line_says_where_a_fallback_reference_came_from() {
    let line = swapped_power_line(
        &power_with(&run_1113b_guest(), None),
        SwappedHitTally::default(),
    );
    assert!(line.contains(
        "reference weapon 13 lv 0 at 179/103/30% (fallback: the weakest one-handed sword of the table)"
    ));
}

#[test]
fn the_hosts_equipped_weapon_is_read_for_sending_unless_the_forced_weapon_is_in_play() {
    let mut memory = run_1113b_guest();
    memory.put_at_rva(
        ENTITY_RVA + SAVE_FORCED_WEAPON_FLAG_OFFSET as u32,
        &0_i32.to_le_bytes(),
    );
    assert_eq!(
        ReferenceWeapon::read_equipped(&memory),
        Some(ReferenceWeapon { id: 13, level: 0 })
    );
    memory.put_at_rva(
        ENTITY_RVA + SAVE_FORCED_WEAPON_FLAG_OFFSET as u32,
        &1_i32.to_le_bytes(),
    );
    assert_eq!(ReferenceWeapon::read_equipped(&memory), None);
}
