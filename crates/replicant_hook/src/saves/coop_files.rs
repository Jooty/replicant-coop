use core::ffi::{CStr, c_char, c_void};
use core::ptr;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

use replicant_game::save_files::{CoopSaveName, CoopSavePaths, coop_save_name};
use replicant_game::story_progress::save_file_is_valid;

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::game_hooks::hook_address;
use crate::saves::write_atomically;

const SAVE_IO_READ_FILE_RVA: u64 = 0x7C5E40;
const SAVE_IO_WRITE_FILE_RVA: u64 = 0x7C6010;
const SAVE_IO_FOLDER_OFFSET: usize = 0x18;
const WRITE_SUCCEEDED: u8 = 0;

static ORIGINAL_READ_FILE: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static ORIGINAL_WRITE_FILE: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static BACKED_UP_THIS_SESSION: AtomicBool = AtomicBool::new(false);
static COOP_SAVE_NAME: OnceLock<CoopSaveName> = OnceLock::new();

struct PreparedWrite {
    replacement: Option<Vec<u8>>,
    paths: CoopSavePaths,
}

type ReadFile = unsafe extern "system" fn(*mut c_void, *const c_char, usize, *mut c_void) -> u64;
type WriteFile = unsafe extern "system" fn(*mut c_void, *const c_char, *const c_void, u32) -> u8;

pub fn writes_are_redirected() -> bool {
    !ORIGINAL_WRITE_FILE.load(Ordering::Acquire).is_null()
}

pub fn install(build: VerifiedBuild, name: CoopSaveName, redirect_writes: bool) {
    log_line(&format!("saves: the co-op file is {}", name.file_name()));
    if COOP_SAVE_NAME.set(name).is_err() {
        log_line("saves: the co-op file hooks were already installed");
        return;
    }
    let read_address = (build.module_base() + SAVE_IO_READ_FILE_RVA) as usize;
    #[expect(unsafe_code, reason = "hooks SaveIo::ReadFile by its address")]
    // SAFETY: the RVA starts SaveIo::ReadFile (io, name, arg, out) -> u64 in the verified build,
    // and the replacement has the same signature.
    unsafe {
        hook_address(
            "SaveIo::ReadFile",
            read_address,
            read_file_detour as *const () as *mut c_void,
            &ORIGINAL_READ_FILE,
        )
    };
    if redirect_writes {
        let write_address = (build.module_base() + SAVE_IO_WRITE_FILE_RVA) as usize;
        #[expect(unsafe_code, reason = "hooks SaveIo::WriteFile by its address")]
        // SAFETY: the RVA starts SaveIo::WriteFile (io, name, data, size) -> char in the verified
        // build, and the replacement has the same signature.
        unsafe {
            hook_address(
                "SaveIo::WriteFile (co-op file)",
                write_address,
                write_file_detour as *const () as *mut c_void,
                &ORIGINAL_WRITE_FILE,
            )
        };
    }
}

#[expect(
    unsafe_code,
    reason = "calls the original ReadFile through the trampoline"
)]
extern "system" fn read_file_detour(
    io: *mut c_void,
    name: *const c_char,
    arg: usize,
    out: *mut c_void,
) -> u64 {
    let original = ORIGINAL_READ_FILE.load(Ordering::Acquire);
    if original.is_null() {
        return 1;
    }
    log_read_target(io, name);
    let name = coop_name_or(name);
    // SAFETY: the trampoline continues into the original ReadFile, which has this signature.
    let result =
        unsafe { core::mem::transmute::<*mut c_void, ReadFile>(original)(io, name, arg, out) };
    log_line(&format!("saves: read returned {result}"));
    result
}

#[expect(
    unsafe_code,
    reason = "calls the original WriteFile through the trampoline"
)]
extern "system" fn write_file_detour(
    io: *mut c_void,
    name: *const c_char,
    data: *const c_void,
    size: u32,
) -> u8 {
    let original = ORIGINAL_WRITE_FILE.load(Ordering::Acquire);
    if original.is_null() {
        return 1;
    }
    let coop_name = coop_name_or(name);
    let prepared = if coop_name == name || data.is_null() {
        None
    } else {
        prepare_coop_write(io, data, size)
    };
    let rewritten = prepared
        .as_ref()
        .and_then(|prepared| prepared.replacement.as_ref());
    let (data, size) = match rewritten {
        Some(file) => (
            file.as_ptr().cast::<c_void>(),
            u32::try_from(file.len()).unwrap_or(size),
        ),
        None => (data, size),
    };
    // SAFETY: the trampoline continues into the original WriteFile, which has this signature;
    // `prepared` outlives the call.
    let result = unsafe {
        core::mem::transmute::<*mut c_void, WriteFile>(original)(io, coop_name, data, size)
    };
    if let Some(prepared) = &prepared {
        let succeeded = result == WRITE_SUCCEEDED;
        crate::story_progress::finish_coop_save(&prepared.paths, succeeded);
        if let Some(net) = crate::net::handle() {
            net.tell_launcher_saved(succeeded);
        }
    }
    result
}

#[expect(
    unsafe_code,
    reason = "reads the save folder inside SaveIo and the buffer the game is writing"
)]
fn prepare_coop_write(io: *mut c_void, data: *const c_void, size: u32) -> Option<PreparedWrite> {
    let paths = COOP_SAVE_NAME.get()?.paths_in(&save_folder(io)?);
    back_up_once_per_session(&paths);
    let length = usize::try_from(size).ok()?;
    // SAFETY: the game passes the buffer it is about to write and its length in bytes.
    let bytes = unsafe { core::slice::from_raw_parts(data.cast::<u8>(), length) };
    let story_rewrite = crate::story_progress::rewrite_coop_save(&paths, bytes);
    let replacement = crate::kaine::guard_coop_save(&paths.file, bytes, story_rewrite);
    Some(PreparedWrite { replacement, paths })
}

#[expect(unsafe_code, reason = "reads the save folder inside SaveIo")]
fn save_folder(io: *mut c_void) -> Option<PathBuf> {
    if io.is_null() {
        return None;
    }
    // SAFETY: SaveIo keeps the save folder as an inline NUL-terminated string at +0x18 (the
    // original formats "%s\\%s" from it); the game checks it is not empty before writing.
    let folder = unsafe { CStr::from_ptr(io.cast::<u8>().add(SAVE_IO_FOLDER_OFFSET).cast()) };
    let folder = PathBuf::from(folder.to_string_lossy().into_owned());
    (!folder.as_os_str().is_empty()).then_some(folder)
}

#[expect(unsafe_code, reason = "reads the file name the game passed in")]
fn log_read_target(io: *mut c_void, name: *const c_char) {
    if name.is_null() {
        return;
    }
    // SAFETY: the game passes a NUL-terminated file name.
    let game_name = unsafe { CStr::from_ptr(name) };
    let Some(folder) = save_folder(io) else {
        log_line(&format!("saves: read of {game_name:?} with no save folder"));
        return;
    };
    let Some(coop) = COOP_SAVE_NAME.get() else {
        return;
    };
    let coop_file = coop.paths_in(&folder).file;
    let size = std::fs::metadata(&coop_file).map(|meta| meta.len());
    log_line(&format!(
        "saves: read of {game_name:?} in {}, co-op file {} ({})",
        folder.display(),
        coop_file.display(),
        match size {
            Ok(bytes) => format!("{bytes} bytes"),
            Err(error) => format!("missing: {error}"),
        }
    ));
}

fn back_up_once_per_session(paths: &CoopSavePaths) {
    if BACKED_UP_THIS_SESSION.load(Ordering::Acquire) {
        return;
    }
    let Ok(current) = std::fs::read(&paths.file) else {
        return;
    };
    if !save_file_is_valid(&current) {
        log_line("saves: the co-op file is not valid, so it is not copied over the backup");
        return;
    }
    match write_atomically(&paths.backup, &current) {
        Ok(()) => BACKED_UP_THIS_SESSION.store(true, Ordering::Release),
        Err(error) => log_line(&format!("saves: cannot back up the co-op file ({error})")),
    }
}

#[expect(unsafe_code, reason = "reads the file name the game passed in")]
fn coop_name_or(name: *const c_char) -> *const c_char {
    let Some(coop) = COOP_SAVE_NAME.get() else {
        return name;
    };
    if name.is_null() {
        return name;
    }
    // SAFETY: the game passes a NUL-terminated file name.
    let game_name = unsafe { CStr::from_ptr(name) };
    match coop_save_name(game_name.to_bytes(), coop) {
        Some(coop_name) => {
            log_line(&format!("saves: using the co-op file for {game_name:?}"));
            coop_name.as_ptr().cast()
        }
        None => {
            log_line(&format!("saves: {game_name:?} stays as the game named it"));
            name
        }
    }
}
