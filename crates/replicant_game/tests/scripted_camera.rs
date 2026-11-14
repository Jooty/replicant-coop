use replicant_game::fight_sync::{
    ScriptedCameraNative, angle_between_degrees, forward_of_world_matrix, look_angles_degrees,
    read_native_numbers,
};
use replicant_game::story_jump::{
    NATIVE_ARG_FIRST_OFFSET_BYTES, NATIVE_ARG_INTEGER_TYPE_TAG, NATIVE_ARG_REAL_TYPE_TAG,
    NATIVE_ARG_STRIDE_BYTES, NATIVE_ARG_VALUE_OFFSET_BYTES, NATIVE_ARGS_HEADER_BYTES,
};

use crate::fake_memory::FakeMemory;

const ARGS: u64 = 0x5_0000;

#[test]
fn every_camera_native_keeps_its_code() {
    for native in ScriptedCameraNative::ALL {
        assert_eq!(ScriptedCameraNative::from_code(native.code()), Some(native));
    }
    assert_eq!(ScriptedCameraNative::from_code(0), None);
}

#[test]
fn the_look_direction_is_column_two_of_the_world_matrix() {
    let mut matrix = [0.0_f32; 16];
    matrix[2] = 3.0;
    matrix[6] = -3.0;
    let forward = forward_of_world_matrix(&matrix).unwrap();
    let (yaw, pitch) = look_angles_degrees(forward);
    assert!((yaw - 90.0).abs() < 0.01 && (pitch + 45.0).abs() < 0.01);
    assert_eq!(forward_of_world_matrix(&[0.0; 16]), None);
    assert!((angle_between_degrees([1.0, 0.0, 0.0], [-2.0, 0.0, 0.0]) - 180.0).abs() < 0.01);
}

#[test]
fn a_natives_numbers_are_read_from_real_and_integer_slots() {
    let mut memory = FakeMemory::new();
    let slot = |index: usize| {
        ARGS + (NATIVE_ARGS_HEADER_BYTES
            + NATIVE_ARG_FIRST_OFFSET_BYTES
            + index * NATIVE_ARG_STRIDE_BYTES) as u64
    };
    let value = NATIVE_ARG_VALUE_OFFSET_BYTES as u64;
    memory.put(slot(0), &NATIVE_ARG_REAL_TYPE_TAG.to_le_bytes());
    memory.put(slot(0) + value, &(-11.5_f64).to_le_bytes());
    memory.put(slot(1), &NATIVE_ARG_INTEGER_TYPE_TAG.to_le_bytes());
    memory.put(slot(1) + value, &3_i32.to_le_bytes());
    memory.put(slot(2), &9_u32.to_le_bytes());
    assert_eq!(
        read_native_numbers(&memory, ARGS, 4),
        vec![Some(-11.5), Some(3.0), None, None]
    );
}
