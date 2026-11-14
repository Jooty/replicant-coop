use core::ffi::c_void;
use core::ptr;
use std::panic;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU16, Ordering};

use replicant_game::Replicant;
use replicant_game::dialogue::{
    TALK_EXPAND_TAGS_RVA, TALK_SPEAKER_NAME_RVA, replace_player_name_tag, speaker_name_to_show,
};
use replicant_game::player_name::NAME_FIELD_LEN;

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::dialogue::hooks::MIRRORING;
use crate::dialogue::tags::mirror_action_running;
use crate::game_hooks::hook_address;
use crate::memory::InProcessMemory;
use crate::net::handle;

const MAX_TAG_TEXT_BYTES: usize = 0x1000;
const MAX_SPEAKER_UNITS: usize = 64;

static HOOKED_BUILD: OnceLock<VerifiedBuild> = OnceLock::new();
static ORIGINAL_EXPAND_TAGS: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static ORIGINAL_SPEAKER_NAME: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static SPEAKER_BUFFER: [AtomicU16; NAME_FIELD_LEN] = [const { AtomicU16::new(0) }; NAME_FIELD_LEN];
static LOGGED_REPLACED_THIS_TALK: AtomicBool = AtomicBool::new(false);
static LOGGED_KEPT_THIS_TALK: AtomicBool = AtomicBool::new(false);

type ExpandTags = unsafe extern "system" fn(*mut c_void, *mut u8, i32, *const u8) -> u64;
type SpeakerName = unsafe extern "system" fn(*mut c_void) -> *const u16;

pub(super) fn install(build: VerifiedBuild) {
    if HOOKED_BUILD.set(build).is_err() {
        return;
    }
    let base = build.module_base();
    #[expect(
        unsafe_code,
        reason = "hooks the talk text tag expander and the talk speaker name getter by address"
    )]
    // SAFETY: both RVAs start the named functions in the verified build (see docs
    // off-hand/dialogue.md), and each replacement has the same signature.
    unsafe {
        hook_address(
            "TalkText::ExpandTags",
            (base + u64::from(TALK_EXPAND_TAGS_RVA)) as usize,
            expand_tags_detour as *const () as *mut c_void,
            &ORIGINAL_EXPAND_TAGS,
        );
        hook_address(
            "TalkPlayer::SpeakerName",
            (base + u64::from(TALK_SPEAKER_NAME_RVA)) as usize,
            speaker_name_detour as *const () as *mut c_void,
            &ORIGINAL_SPEAKER_NAME,
        );
    }
}

pub(super) fn begin_talk() {
    LOGGED_REPLACED_THIS_TALK.store(false, Ordering::Relaxed);
    LOGGED_KEPT_THIS_TALK.store(false, Ordering::Relaxed);
}

fn partner_name_while_mirroring() -> Option<String> {
    if !(MIRRORING.load(Ordering::Acquire) || mirror_action_running()) {
        return None;
    }
    handle()?.view().partner_name().map(str::to_owned)
}

#[expect(
    unsafe_code,
    reason = "reads the game's NUL-terminated line text and calls the original expander through the trampoline"
)]
extern "system" fn expand_tags_detour(
    context: *mut c_void,
    out: *mut u8,
    size: i32,
    text: *const u8,
) -> u64 {
    let original = ORIGINAL_EXPAND_TAGS.load(Ordering::Acquire);
    if original.is_null() {
        return 0;
    }
    let call_original = |text: *const u8| {
        // SAFETY: the trampoline continues into the original expander, which has this signature.
        unsafe {
            core::mem::transmute::<*mut c_void, ExpandTags>(original)(context, out, size, text)
        }
    };
    if text.is_null() {
        return call_original(text);
    }
    let replacement = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let partner = partner_name_while_mirroring()?;
        let line = read_line_text(text)?;
        replace_player_name_tag(&line, &partner)
    }))
    .unwrap_or(None);
    match replacement {
        Some(mut replaced) => {
            replaced.push(0);
            call_original(replaced.as_ptr())
        }
        None => call_original(text),
    }
}

#[expect(unsafe_code, reason = "reads the game's NUL-terminated line text")]
fn read_line_text(text: *const u8) -> Option<Vec<u8>> {
    let mut line = Vec::new();
    for offset in 0..MAX_TAG_TEXT_BYTES {
        // SAFETY: the game's line text is NUL-terminated and the scan stops at the cap.
        let byte = unsafe { text.add(offset).read() };
        if byte == 0 {
            return Some(line);
        }
        line.push(byte);
    }
    None
}

#[expect(
    unsafe_code,
    reason = "reads the game's NUL-terminated UTF-16 speaker name and calls the original getter through the trampoline"
)]
extern "system" fn speaker_name_detour(player: *mut c_void) -> *const u16 {
    let original = ORIGINAL_SPEAKER_NAME.load(Ordering::Acquire);
    if original.is_null() {
        return ptr::null();
    }
    // SAFETY: the trampoline continues into the original getter, which has this signature.
    let shown = unsafe { core::mem::transmute::<*mut c_void, SpeakerName>(original)(player) };
    if shown.is_null() || partner_name_while_mirroring().is_none() {
        return shown;
    }
    let mut units = Vec::new();
    for offset in 0..MAX_SPEAKER_UNITS {
        // SAFETY: the speaker name is a NUL-terminated UTF-16 string and the scan stops at the cap.
        let unit = unsafe { shown.add(offset).read() };
        if unit == 0 {
            break;
        }
        units.push(unit);
    }
    panic::catch_unwind(panic::AssertUnwindSafe(|| {
        choose_speaker_name(shown, &String::from_utf16_lossy(&units))
    }))
    .unwrap_or(shown)
}

fn choose_speaker_name(shown: *const u16, shown_text: &str) -> *const u16 {
    let own_name = HOOKED_BUILD
        .get()
        .and_then(|build| Replicant::new(InProcessMemory::new(build)).save_name().ok())
        .unwrap_or_default();
    let partner = partner_name_while_mirroring();
    let Some(name) = speaker_name_to_show(shown_text, &own_name, partner.as_deref()) else {
        if !LOGGED_KEPT_THIS_TALK.swap(true, Ordering::Relaxed) {
            log_line(&format!(
                "dialogue: speaker \"{shown_text}\" kept (your save name is \"{own_name}\")"
            ));
        }
        return shown;
    };
    let mut written = 0;
    for (slot, unit) in SPEAKER_BUFFER.iter().zip(name.encode_utf16()) {
        slot.store(unit, Ordering::Relaxed);
        written += 1;
    }
    SPEAKER_BUFFER[written.min(NAME_FIELD_LEN - 1)].store(0, Ordering::Relaxed);
    if !LOGGED_REPLACED_THIS_TALK.swap(true, Ordering::Relaxed) {
        log_line(&format!(
            "dialogue: speaker shown as {name} (the game had \"{shown_text}\")"
        ));
    }
    SPEAKER_BUFFER[0].as_ptr().cast_const()
}
