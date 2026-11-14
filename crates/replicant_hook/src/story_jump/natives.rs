use replicant_game::story_jump::{
    NATIVE_ARG_FIRST_OFFSET_BYTES, NATIVE_ARG_INTEGER_TYPE_TAG, NATIVE_ARG_REAL_TYPE_TAG,
    NATIVE_ARG_STRIDE_BYTES, NATIVE_ARG_TEXT_CAPACITY_BYTES, NATIVE_ARG_TEXT_OFFSET_BYTES,
    NATIVE_ARG_TEXT_TYPE_TAG, NATIVE_ARG_VALUE_OFFSET_BYTES, NATIVE_ARGS_HEADER_BYTES,
};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;

const BYTES_PER_WORD: usize = 8;

type LuaNative = unsafe extern "system" fn(*mut u8);

#[derive(Debug, Clone, Copy)]
pub(crate) enum NativeArg {
    Integer(i32),
    Real(f64),
    Text(&'static str),
}

pub(super) fn call_native(build: &VerifiedBuild, rva: u32, integers: &[i32]) {
    let arguments: Vec<NativeArg> = integers.iter().map(|v| NativeArg::Integer(*v)).collect();
    call_native_with(build, rva, &arguments);
}

#[expect(
    unsafe_code,
    reason = "calls a game script native at its verified address with a hand-built argument block"
)]
pub(crate) fn call_native_with(build: &VerifiedBuild, rva: u32, arguments: &[NativeArg]) {
    crate::teleport::note_mod_move();
    let bytes = NATIVE_ARGS_HEADER_BYTES
        + NATIVE_ARG_FIRST_OFFSET_BYTES
        + arguments.len() * NATIVE_ARG_STRIDE_BYTES
        + NATIVE_ARG_VALUE_OFFSET_BYTES
        + BYTES_PER_WORD;
    let mut block = vec![0_u64; bytes.div_ceil(BYTES_PER_WORD)];
    let base = block.as_mut_ptr().cast::<u8>();
    for (index, value) in arguments.iter().enumerate() {
        let argument = NATIVE_ARGS_HEADER_BYTES
            + NATIVE_ARG_FIRST_OFFSET_BYTES
            + index * NATIVE_ARG_STRIDE_BYTES;
        // SAFETY: `block` holds the header plus every argument slot, so the writes stay inside it,
        // and the offsets are 4-byte aligned (8-byte for the real) from the 8-byte aligned start.
        unsafe {
            let slot = base.add(argument);
            let value_slot = base.add(argument + NATIVE_ARG_VALUE_OFFSET_BYTES);
            match *value {
                NativeArg::Integer(integer) => {
                    slot.cast::<u32>().write(NATIVE_ARG_INTEGER_TYPE_TAG);
                    value_slot.cast::<i32>().write(integer);
                }
                NativeArg::Real(real) => {
                    slot.cast::<u32>().write(NATIVE_ARG_REAL_TYPE_TAG);
                    value_slot.cast::<f64>().write(real);
                }
                NativeArg::Text(text) => {
                    // The block is zeroed, so a text cut one byte short of its room ends in NUL.
                    let length = text.len().min(NATIVE_ARG_TEXT_CAPACITY_BYTES - 1);
                    slot.cast::<u32>().write(NATIVE_ARG_TEXT_TYPE_TAG);
                    core::ptr::copy_nonoverlapping(
                        text.as_ptr(),
                        base.add(argument + NATIVE_ARG_TEXT_OFFSET_BYTES),
                        length,
                    );
                }
            }
        }
    }
    let function = (build.module_base() + u64::from(rva)) as usize;
    // SAFETY: the build gate passed, so the RVA starts the native that Ghidra named there; it reads
    // its integer arguments from the block built above and this runs on the game thread inside the
    // script manager tick, where the scripts call natives.
    unsafe {
        log_line(&format!("native call: rva 0x{rva:X} start"));
        core::mem::transmute::<usize, LuaNative>(function)(base);
        log_line(&format!("native call: rva 0x{rva:X} end"));
    }
}
