use core::ffi::{c_char, c_void};
use core::ptr;
use std::ops::Range;
use std::panic;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock, PoisonError};

use replicant_game::effects::{EffectSource, ProbeLogLimiter, read_sound};
use replicant_game::offsets::{SOUND_BY_ID_FUNCTION_RVA, SOUND_BY_ID_WRAPPER_FUNCTION_RVA};
use replicant_game::{GameMemory, Replicant};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::effects::hooks::{game_caller_rva, probe_window_open, source_in_play};
use crate::game_hooks::hook_address;
use crate::memory::InProcessMemory;
use crate::net::net_clock_ms;
use crate::puppet::is_puppet_actor;

static HOOKED_BUILD: OnceLock<VerifiedBuild> = OnceLock::new();
static ORIGINAL_SOUND_BY_ID: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static ORIGINAL_SOUND_BY_ID_WRAPPER: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static PROBE_LOG: LazyLock<Mutex<ProbeLogLimiter>> = LazyLock::new(Mutex::default);

const SOUND_BY_ID_WRAPPER_BODY_RVAS: Range<u32> = 0x3C_A9F0..0x3C_AA27;

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
type SoundByIdWrapper = unsafe extern "system" fn(*mut c_void, u32, *const f32, f32, f32) -> u8;

pub(super) fn install(build: VerifiedBuild) {
    if HOOKED_BUILD.set(build).is_err() {
        return;
    }
    let base = build.module_base();
    #[expect(unsafe_code, reason = "hooks the play-sound-by-id function by address")]
    // SAFETY: the RVA starts the function (sound manager, sound id, position pointer, float, two
    // floats, byte, pointer -> pointer) in the verified build, and the replacement matches it.
    unsafe {
        hook_address(
            "SoundMap::PlayById",
            (base + u64::from(SOUND_BY_ID_FUNCTION_RVA)) as usize,
            sound_by_id_detour as *const () as *mut c_void,
            &ORIGINAL_SOUND_BY_ID,
        )
    };
    #[expect(
        unsafe_code,
        reason = "hooks the play-sound-by-id wrapper function by address"
    )]
    // SAFETY: the RVA starts the wrapper (sound manager, sound id, position pointer, float, float
    // -> bool) that calls the function above, and the replacement matches it.
    unsafe {
        hook_address(
            "SoundMap::PlayByIdWrapper",
            (base + u64::from(SOUND_BY_ID_WRAPPER_FUNCTION_RVA)) as usize,
            sound_by_id_wrapper_detour as *const () as *mut c_void,
            &ORIGINAL_SOUND_BY_ID_WRAPPER,
        )
    };
}

fn probe_actor_sound(actor: u64, name: u64) {
    let Some((game, source)) = source_in_play(actor) else {
        return;
    };
    let puppet = is_puppet_actor(actor);
    if source.sound_route().is_some() && !puppet {
        return;
    }
    let sound = read_sound(game.memory(), name).ok().flatten();
    let sound = sound.as_deref().unwrap_or("?");
    log_probe(|| {
        format!(
            "sound probe: {sound} played by {}",
            source_kind(&game, actor, source, puppet)
        )
    });
}

fn source_kind(
    game: &Replicant<InProcessMemory>,
    actor: u64,
    source: EffectSource,
    puppet: bool,
) -> String {
    if puppet {
        return "the partner's puppet".to_owned();
    }
    let memory = game.memory();
    match memory.read_u64(actor) {
        Ok(vtable) => format!(
            "{source:?} (class rva {:#x})",
            vtable.wrapping_sub(memory.module_base())
        ),
        Err(_) => format!("{source:?} (class unreadable)"),
    }
}

fn log_probe(line: impl FnOnce() -> String) {
    let admitted = PROBE_LOG
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .admit(net_clock_ms());
    let Some(dropped) = admitted else {
        return;
    };
    if dropped > 0 {
        log_line(&format!(
            "sound probe: {dropped} lines were dropped by the rate limit"
        ));
    }
    log_line(&line());
}

fn log_by_id_sound(sound_id: u32, position: u64, caller_rva: u32) {
    let Some(build) = HOOKED_BUILD.get() else {
        return;
    };
    let memory = InProcessMemory::new(build);
    let coordinate = |index: u64| memory.read_f32(position + index * 4).ok();
    let (x, y, z) = (coordinate(0), coordinate(1), coordinate(2));
    log_probe(|| match (x, y, z) {
        (Some(x), Some(y), Some(z)) => format!(
            "sound probe: by-id sound {sound_id} at ({x:.2}, {y:.2}, {z:.2}) from {caller_rva:#x}"
        ),
        _ => format!(
            "sound probe: by-id sound {sound_id} at (unreadable position) from {caller_rva:#x}"
        ),
    });
}

#[expect(
    unsafe_code,
    reason = "calls the original play-sound-by-id function through the trampoline MinHook built"
)]
extern "system" fn sound_by_id_detour(
    manager: *mut c_void,
    sound_id: u32,
    position: *const f32,
    fourth: f32,
    fifth: f32,
    sixth: f32,
    seventh: u8,
    eighth: usize,
) -> *mut c_void {
    let original = ORIGINAL_SOUND_BY_ID.load(Ordering::Acquire);
    if original.is_null() {
        return ptr::null_mut();
    }
    if probe_window_open() {
        let position = position as usize;
        let _ = panic::catch_unwind(|| {
            let caller_rva = game_caller_rva();
            if !SOUND_BY_ID_WRAPPER_BODY_RVAS.contains(&caller_rva) {
                log_by_id_sound(sound_id, position as u64, caller_rva);
            }
        });
    }
    // SAFETY: the trampoline continues into the original function, which has this signature.
    let result = unsafe {
        core::mem::transmute::<*mut c_void, SoundById>(original)(
            manager, sound_id, position, fourth, fifth, sixth, seventh, eighth,
        )
    };
    let position = position as usize;
    let _ = panic::catch_unwind(|| note_shot_sound(sound_id, position as u64, [fourth, fifth]));
    result
}

fn note_shot_sound(sound_id: u32, position: u64, levels: [f32; 2]) {
    let Some(build) = HOOKED_BUILD.get() else {
        return;
    };
    if position == 0 {
        return;
    }
    let memory = InProcessMemory::new(build);
    let coordinate = |index: u64| memory.read_f32(position + index * 4);
    if let (Ok(x), Ok(y), Ok(z)) = (coordinate(0), coordinate(1), coordinate(2)) {
        crate::enemy_bullets::note_sound_made(sound_id, [x, y, z], levels);
    }
}

#[expect(
    unsafe_code,
    reason = "calls the original play-sound-by-id wrapper through the trampoline MinHook built"
)]
extern "system" fn sound_by_id_wrapper_detour(
    manager: *mut c_void,
    sound_id: u32,
    position: *const f32,
    fourth: f32,
    fifth: f32,
) -> u8 {
    let original = ORIGINAL_SOUND_BY_ID_WRAPPER.load(Ordering::Acquire);
    if original.is_null() {
        return 0;
    }
    if probe_window_open() {
        let position = position as usize;
        let _ = panic::catch_unwind(|| {
            log_by_id_sound(sound_id, position as u64, game_caller_rva());
        });
    }
    // SAFETY: the trampoline continues into the original function, which has this signature.
    unsafe {
        core::mem::transmute::<*mut c_void, SoundByIdWrapper>(original)(
            manager, sound_id, position, fourth, fifth,
        )
    }
}

pub(super) fn probe_play_sound(actor: *mut c_void, name: *const c_char) {
    if !probe_window_open() {
        return;
    }
    let (actor, name) = (actor as usize, name as usize);
    let _ = panic::catch_unwind(|| probe_actor_sound(actor as u64, name as u64));
}
