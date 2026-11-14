use core::ffi::{c_char, c_void};
use std::ffi::CString;

use replicant_game::effects::{
    build_effect_call, effect_instance_address, match_anchor_offset, wwise_name_hash,
};
use replicant_game::offsets::{
    ACTOR_PLAY_EFFECT_VTABLE_OFFSET, ACTOR_SOUND_EMITTER_OFFSET, ACTOR_TRANSFORM_BLOCK_OFFSET,
    ACTOR_TRANSFORM_ROW_STRIDE, EFFECT_INSTANCE_CHANGED_FLAG_OFFSET, EFFECT_INSTANCE_MATRIX_OFFSET,
    EFFECT_INVALID_HANDLE, EFFECT_SPAWN_BY_NAME_FUNCTION_RVA, SOUND_EMITTER_PLAY_FUNCTION_RVA,
    SOUND_SET_SWITCH_FUNCTION_RVA,
};
use replicant_game::{GameMemory, Replicant};
use replicant_proto::{EffectPlay, WORLD_EFFECT_TRANSFORM_FLOATS, WorldEffectPlay};

use crate::build_gate::VerifiedBuild;
use crate::effects::hooks::nested_effect_call;
use crate::kaine::play_for_partner;
use crate::memory::InProcessMemory;

type PlayEffect =
    unsafe extern "system" fn(*mut c_void, *mut u32, *const u8, *const u8) -> *mut u32;
type SpawnByName =
    unsafe extern "system" fn(*mut u32, *const u8, *const f32, f32, u32, u32, u32, i32) -> *mut u32;
type PlaySound =
    unsafe extern "system" fn(*mut c_void, *const c_char, f32, i32, i32) -> *mut c_void;

type SetSwitch = unsafe extern "system" fn(*mut c_void, *const u32, *const u32) -> u64;

pub(super) struct PlayedEffect {
    pub handle: Option<u32>,
    pub block: Vec<u64>,
}

#[repr(align(16))]
struct AlignedTransform([f32; WORLD_EFFECT_TRANSFORM_FLOATS]);

pub(super) fn play_effect(
    build: &VerifiedBuild,
    actor: u64,
    effect: &EffectPlay,
) -> Result<PlayedEffect, String> {
    let memory = InProcessMemory::new(build);
    let vtable = memory.read_u64(actor).map_err(|error| error.to_string())?;
    let function = memory
        .read_u64(vtable + ACTOR_PLAY_EFFECT_VTABLE_OFFSET)
        .map_err(|error| error.to_string())?;
    if function < build.module_base() {
        return Err(format!("the play-effect slot holds {function:#x}"));
    }
    let call = build_effect_call(effect);
    let block = aligned_words(&call.bytes);
    let params = block.as_ptr().cast::<u8>();
    let attach = params.wrapping_add(call.attach_offset);
    let mut handle = EFFECT_INVALID_HANDLE;
    #[expect(unsafe_code, reason = "calls the actor's play-effect virtual function")]
    // SAFETY: the pointer was read from the actor's vtable and lies inside the game image; the
    // slot takes (actor, out handle, effect parameters, attach descriptor) as the game's own
    // callers pass them. The blocks live in `block`, which the caller keeps for the effect's life.
    play_for_partner(|| unsafe {
        core::mem::transmute::<usize, PlayEffect>(function as usize)(
            actor as usize as *mut c_void,
            &mut handle,
            params,
            attach,
        );
    });
    Ok(PlayedEffect {
        handle: (handle != EFFECT_INVALID_HANDLE).then_some(handle),
        block,
    })
}

pub(super) fn align_anchor_offset(build: &VerifiedBuild, puppet: u64) -> Option<[f32; 3]> {
    let player = Replicant::new(InProcessMemory::new(build))
        .playable_actor()
        .ok()?;
    match_anchor_offset(&mut InProcessMemory::new(build), player, puppet).ok()
}

pub(super) fn play_sound(build: &VerifiedBuild, actor: u64, sound: &str, volume: f32) -> bool {
    let Ok(name) = CString::new(sound) else {
        return false;
    };
    let function = (build.module_base() + u64::from(SOUND_EMITTER_PLAY_FUNCTION_RVA)) as usize;
    let emitter = actor + ACTOR_SOUND_EMITTER_OFFSET;
    #[expect(
        unsafe_code,
        reason = "calls the sound emitter's play function at its verified address"
    )]
    // SAFETY: the RVA starts the function (emitter, sound name, volume, 0, 0 -> voice or null)
    // in the verified build; the emitter is the actor's own, the name is NUL terminated and
    // outlives the call, and this runs on the game thread inside the script manager tick.
    let result = unsafe {
        core::mem::transmute::<usize, PlaySound>(function)(
            emitter as usize as *mut c_void,
            name.as_ptr(),
            volume,
            0,
            0,
        )
    };
    !result.is_null()
}

pub(super) fn set_switch(build: &VerifiedBuild, actor: u64, group: &str, state: &str) -> bool {
    let function = (build.module_base() + u64::from(SOUND_SET_SWITCH_FUNCTION_RVA)) as usize;
    let emitter = actor + ACTOR_SOUND_EMITTER_OFFSET;
    let (group_hash, state_hash) = (wwise_name_hash(group), wwise_name_hash(state));
    #[expect(
        unsafe_code,
        reason = "calls the game's set-switch wrapper at its verified address"
    )]
    // SAFETY: the RVA starts the function (emitter, group id pointer, state id pointer -> bool
    // in the low byte) in the verified build; it reads the two ids during the call, both outlive
    // it, the emitter is the actor's own, and this runs on the game thread.
    let result = unsafe {
        core::mem::transmute::<usize, SetSwitch>(function)(
            emitter as usize as *mut c_void,
            &group_hash,
            &state_hash,
        )
    };
    result & 0xFF != 0
}

pub(super) fn distance_to_listener(build: &VerifiedBuild, actor: u64) -> Option<f32> {
    let game = Replicant::new(InProcessMemory::new(build));
    let listener = game.position().ok()?;
    let memory = game.memory();
    let base = actor + ACTOR_TRANSFORM_BLOCK_OFFSET;
    let row = ACTOR_TRANSFORM_ROW_STRIDE as u64;
    let lane = 12;
    let (x, y, z) = (
        memory.read_f32(base + lane).ok()?,
        memory.read_f32(base + row + lane).ok()?,
        memory.read_f32(base + 2 * row + lane).ok()?,
    );
    Some(((x - listener.x).powi(2) + (y - listener.y).powi(2) + (z - listener.z).powi(2)).sqrt())
}

pub(super) fn spawn_world_effect(build: &VerifiedBuild, effect: &WorldEffectPlay) -> Option<u32> {
    let name = CString::new(effect.effect.as_str()).ok()?;
    let transform = AlignedTransform(effect.transform);
    let [scale, fifth, sixth, seventh, eighth] = effect.arguments;
    let function = (build.module_base() + u64::from(EFFECT_SPAWN_BY_NAME_FUNCTION_RVA)) as usize;
    let mut handle = EFFECT_INVALID_HANDLE;
    #[expect(unsafe_code, reason = "calls the game's spawn-effect-by-name function")]
    // SAFETY: the RVA starts the function (out handle, name, 3x4 matrix, scale, four stack words
    // -> out handle) in the verified build; it hashes the name and copies the matrix during the
    // call, both outlive it, the matrix is 16-byte aligned, and this runs on the game thread.
    play_for_partner(|| {
        nested_effect_call(|| unsafe {
            core::mem::transmute::<usize, SpawnByName>(function)(
                &mut handle,
                name.as_ptr().cast::<u8>(),
                transform.0.as_ptr(),
                f32::from_bits(scale),
                fifth,
                sixth,
                seventh,
                eighth.cast_signed(),
            );
        });
    });
    let memory = InProcessMemory::new(build);
    effect_instance_address(&memory, handle)
        .ok()
        .flatten()
        .map(|_| handle)
}

pub(super) fn move_world_effect(
    build: &VerifiedBuild,
    handle: u32,
    transform: &[f32; WORLD_EFFECT_TRANSFORM_FLOATS],
) {
    let mut memory = InProcessMemory::new(build);
    let Ok(Some(instance)) = effect_instance_address(&memory, handle) else {
        return;
    };
    let bytes: Vec<u8> = transform
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    if memory
        .write(instance + EFFECT_INSTANCE_MATRIX_OFFSET, &bytes)
        .is_ok()
    {
        let _ = memory.write_u8(instance + EFFECT_INSTANCE_CHANGED_FLAG_OFFSET, 1);
    }
}

fn aligned_words(bytes: &[u8]) -> Vec<u64> {
    bytes
        .chunks(8)
        .map(|chunk| {
            let mut word = [0_u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            u64::from_le_bytes(word)
        })
        .collect()
}
