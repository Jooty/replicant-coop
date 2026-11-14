//! The guest's calls into the game's sound code for the sounds a host's shot made.

use core::ffi::c_void;

use replicant_game::GameMemory;
use replicant_game::offsets::{
    AUDIO_MAN_RVA, EFFECT_CREATE_FADE_ARGUMENT_RVA, SOUND_BY_ID_FUNCTION_RVA,
    SOUND_IS_PLAYING_FUNCTION_RVA, SOUND_SET_POSITION_FUNCTION_RVA, SOUND_STOP_FUNCTION_RVA,
};
use replicant_proto::SHOT_SOUND_LEVELS;

use crate::build_gate::VerifiedBuild;
use crate::memory::InProcessMemory;

type SoundById = unsafe extern "system" fn(
    *mut c_void,
    u32,
    *const f32,
    f32,
    f32,
    f32,
    u8,
    usize,
) -> *mut c_void;
type SoundStop = unsafe extern "system" fn(*mut c_void, *mut c_void, f32);
type SoundIsPlaying = unsafe extern "system" fn(*mut c_void) -> u8;
type SoundSetPosition = unsafe extern "system" fn(*mut c_void, *const f32);

#[repr(align(16))]
struct AlignedPoint([f32; 4]);

/// Plays sound `id` at `position` as a bullet script command does; the game's handle for it,
/// which a lasting sound needs to be moved and stopped.
pub(super) fn play_shot_sound(
    build: &VerifiedBuild,
    id: u32,
    position: [f32; 3],
    levels: [f32; SHOT_SOUND_LEVELS],
) -> Option<u64> {
    let manager = built_audio_manager(build)?;
    let point = AlignedPoint([position[0], position[1], position[2], 1.0]);
    let function = (build.module_base() + u64::from(SOUND_BY_ID_FUNCTION_RVA)) as usize;
    #[expect(unsafe_code, reason = "calls the game's play-sound-by-id function")]
    // SAFETY: the RVA starts SoundMap::PlayById (audio manager, sound id, position, three floats,
    // byte, pointer -> sound) in the verified build, called as bullet script command 0x45 calls
    // it, with the audio manager singleton just checked to be built; on the game thread.
    let sound = unsafe {
        core::mem::transmute::<usize, SoundById>(function)(
            manager,
            id,
            point.0.as_ptr(),
            levels[0],
            levels[1],
            0.0,
            1,
            0,
        )
    };
    (!sound.is_null()).then_some(sound as u64)
}

/// Moves a lasting sound with its copy; `false` once the game no longer plays it.
pub(super) fn move_shot_sound(build: &VerifiedBuild, sound: u64, position: [f32; 3]) -> bool {
    if !sound_is_playing(build, sound) {
        return false;
    }
    let point = AlignedPoint([position[0], position[1], position[2], 1.0]);
    let function = (build.module_base() + u64::from(SOUND_SET_POSITION_FUNCTION_RVA)) as usize;
    #[expect(unsafe_code, reason = "calls the game's set-sound-position function")]
    // SAFETY: the RVA starts SoundHandle::SetPosition (sound, position4) in the verified build,
    // which the bullet's effect loop calls each frame on its own lasting sound; `sound` came
    // from PlayById and the game still plays it; on the game thread.
    unsafe {
        core::mem::transmute::<usize, SoundSetPosition>(function)(
            sound as *mut c_void,
            point.0.as_ptr(),
        );
    }
    true
}

pub(super) fn stop_shot_sound(build: &VerifiedBuild, sound: u64) {
    let Some(manager) = built_audio_manager(build) else {
        return;
    };
    let memory = InProcessMemory::new(build);
    let Ok(fade) =
        memory.read_f32(build.module_base() + u64::from(EFFECT_CREATE_FADE_ARGUMENT_RVA))
    else {
        return;
    };
    let function = (build.module_base() + u64::from(SOUND_STOP_FUNCTION_RVA)) as usize;
    #[expect(unsafe_code, reason = "calls the game's stop-sound function")]
    // SAFETY: the RVA starts CAudioMan::StopSound (audio manager, sound, fade) in the verified
    // build, called with the same fade global as bullet script command 0x46; it checks the
    // sound is still playing before it stops it; on the game thread.
    unsafe {
        core::mem::transmute::<usize, SoundStop>(function)(manager, sound as *mut c_void, fade);
    }
}

fn sound_is_playing(build: &VerifiedBuild, sound: u64) -> bool {
    if sound == 0 {
        return false;
    }
    let function = (build.module_base() + u64::from(SOUND_IS_PLAYING_FUNCTION_RVA)) as usize;
    #[expect(unsafe_code, reason = "calls the game's sound-is-playing check")]
    // SAFETY: the RVA starts the check CAudioMan::StopSound makes on a sound before it stops it
    // (sound -> bool) in the verified build; `sound` came from PlayById; on the game thread.
    let playing =
        unsafe { core::mem::transmute::<usize, SoundIsPlaying>(function)(sound as *mut c_void) };
    playing != 0
}

fn built_audio_manager(build: &VerifiedBuild) -> Option<*mut c_void> {
    let address = build.module_base() + u64::from(AUDIO_MAN_RVA);
    let built = InProcessMemory::new(build)
        .read_u64(address)
        .is_ok_and(|first_word| first_word != 0);
    built.then_some(address as usize as *mut c_void)
}
