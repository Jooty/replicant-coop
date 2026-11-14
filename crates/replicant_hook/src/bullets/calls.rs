use std::sync::{Mutex, PoisonError};

use replicant_game::GameMemory;
use replicant_game::bullet_flights::{partner_effect_id, style_is_set};
use replicant_game::effects::{effect_instance_address, finish_effect};
use replicant_game::offsets::{
    EFFECT_CREATE_BY_ID_FUNCTION_RVA, EFFECT_CREATE_FADE_ARGUMENT_RVA,
    EFFECT_INSTANCE_CHANGED_FLAG_OFFSET, EFFECT_INSTANCE_MATRIX_OFFSET,
    EFFECT_INSTANCE_STYLE_OFFSET, EFFECT_INVALID_HANDLE,
};
use replicant_proto::{BULLET_STYLE_FLOATS, BULLET_TRANSFORM_FLOATS};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::kaine::play_for_partner;
use crate::memory::InProcessMemory;

const MAX_REFUSED_IDS_LOGGED: usize = 64;

static REFUSED_IDS: Mutex<Vec<i32>> = Mutex::new(Vec::new());

type CreateEffectById =
    unsafe extern "system" fn(*mut u32, u32, *const f32, u32, u32, u32, u32) -> *mut u32;

#[repr(align(16))]
struct AlignedMatrix([f32; 16]);

pub(crate) fn create_effect(
    build: &VerifiedBuild,
    effect_id: i32,
    transform: &[f32; BULLET_TRANSFORM_FLOATS],
) -> Option<u32> {
    let Some(id) = partner_effect_id(effect_id) else {
        if effect_id != 0 {
            note_refused_id(effect_id, "outside the range of effect ids");
        }
        return None;
    };
    let memory = InProcessMemory::new(build);
    let fade = memory
        .read_i32(build.module_base() + u64::from(EFFECT_CREATE_FADE_ARGUMENT_RVA))
        .ok()?
        .cast_unsigned();
    let matrix = AlignedMatrix(creation_matrix(transform));
    let function = (build.module_base() + u64::from(EFFECT_CREATE_BY_ID_FUNCTION_RVA)) as usize;
    let mut handle = EFFECT_INVALID_HANDLE;
    #[expect(unsafe_code, reason = "calls the game's create-effect-by-id function")]
    // SAFETY: the RVA starts the function (out handle, effect id, transform matrix, 0, fade,
    // fade, 0 -> out handle) in the verified build, as the bullet's own start code calls it; the
    // matrix is 16-byte aligned and outlives the call, and this runs on the game thread.
    play_for_partner(|| unsafe {
        core::mem::transmute::<usize, CreateEffectById>(function)(
            &mut handle,
            id,
            matrix.0.as_ptr(),
            0,
            fade,
            fade,
            0,
        );
    });
    let created = effect_instance_address(&memory, handle)
        .ok()
        .flatten()
        .is_some();
    if !created {
        note_refused_id(effect_id, "the game created no effect for it");
    }
    created.then_some(handle)
}

pub(crate) fn place_effect(
    build: &VerifiedBuild,
    handle: u32,
    transform: &[f32; BULLET_TRANSFORM_FLOATS],
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

pub(crate) fn style_effect(build: &VerifiedBuild, handle: u32, style: &[f32; BULLET_STYLE_FLOATS]) {
    if !style_is_set(style) {
        return;
    }
    let mut memory = InProcessMemory::new(build);
    let Ok(Some(instance)) = effect_instance_address(&memory, handle) else {
        return;
    };
    let bytes: Vec<u8> = style.iter().flat_map(|value| value.to_le_bytes()).collect();
    if memory
        .write(instance + EFFECT_INSTANCE_STYLE_OFFSET, &bytes)
        .is_ok()
    {
        let _ = memory.write_u8(instance + EFFECT_INSTANCE_CHANGED_FLAG_OFFSET, 1);
    }
}

pub(crate) fn end_effect(build: &VerifiedBuild, handle: u32) {
    let mut memory = InProcessMemory::new(build);
    let _ = finish_effect(&mut memory, handle);
}

fn creation_matrix(transform: &[f32; BULLET_TRANSFORM_FLOATS]) -> [f32; 16] {
    let translation = [transform[3], transform[7], transform[11]];
    [
        transform[0],
        transform[1],
        transform[2],
        translation[0],
        transform[4],
        transform[5],
        transform[6],
        translation[1],
        transform[8],
        transform[9],
        transform[10],
        translation[2],
        translation[0],
        translation[1],
        translation[2],
        1.0,
    ]
}

fn note_refused_id(effect_id: i32, why: &str) {
    let mut refused = REFUSED_IDS.lock().unwrap_or_else(PoisonError::into_inner);
    if refused.contains(&effect_id) || refused.len() >= MAX_REFUSED_IDS_LOGGED {
        return;
    }
    refused.push(effect_id);
    log_line(&format!(
        "bullets: partner effect id {effect_id:#x} refused ({why})"
    ));
}
