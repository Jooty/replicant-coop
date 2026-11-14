use std::thread;
use std::time::Duration;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, STILL_ACTIVE};
use windows_sys::Win32::System::Diagnostics::Debug::{
    CONTEXT, CONTEXT_CONTROL_AMD64, CONTEXT_INTEGER_AMD64, GetThreadContext, ReadProcessMemory,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetExitCodeThread, OpenThread, ResumeThread, SuspendThread,
    THREAD_GET_CONTEXT, THREAD_QUERY_LIMITED_INFORMATION, THREAD_SUSPEND_RESUME,
};

use crate::crash_log::handlers::{frames_in_stack_words, module_at, write_durable_line};
use crate::crash_report::CRASH_REPORT_STACK_SCAN_WORDS;
use crate::diagnostics::log_line;
use crate::stall_watch::{
    STALL_WATCH_INTERVAL_MS, StallSample, StallVerdict, StallWatch, game_thread_ended_line,
    game_thread_heartbeat, game_thread_steps, stall_clock_ms, stall_recovered_line,
    stall_sample_text, stall_unreadable_line,
};

const SUSPEND_FAILED: u32 = u32::MAX;

#[repr(C, align(16))]
struct AlignedContext(CONTEXT);

struct FrozenView {
    instruction: u64,
    stack_words: Vec<u64>,
}

pub(super) fn start() {
    let watcher = thread::Builder::new()
        .name("game-thread-stall-watch".to_owned())
        .spawn(watch_the_game_thread);
    if watcher.is_err() {
        log_line("stall watch: the watcher thread did not start");
    }
}

fn watch_the_game_thread() {
    let mut watch = StallWatch::new();
    let mut ended_reported = false;
    loop {
        thread::sleep(Duration::from_millis(STALL_WATCH_INTERVAL_MS));
        let heartbeat = game_thread_heartbeat();
        let verdict = watch.check(heartbeat.map(|beat| beat.last_tick_ms), stall_clock_ms());
        let Some(heartbeat) = heartbeat else {
            continue;
        };
        match verdict {
            StallVerdict::Ticking | StallVerdict::Waiting => {}
            StallVerdict::Recovered {
                stalled_ms,
                samples,
            } => {
                ended_reported = false;
                write_durable_line(&stall_recovered_line(stalled_ms, samples));
            }
            StallVerdict::Sample { stalled_ms, index } => {
                for line in
                    sample_lines(heartbeat.thread_id, stalled_ms, index, &mut ended_reported)
                {
                    write_durable_line(&line);
                }
            }
        }
    }
}

fn sample_lines(
    thread_id: u32,
    stalled_ms: u64,
    index: u32,
    ended_reported: &mut bool,
) -> Vec<String> {
    let handle = match open_game_thread(thread_id) {
        Ok(handle) => handle,
        Err(why) => return vec![stall_unreadable_line(thread_id, stalled_ms, &why)],
    };
    let lines = match exit_code(handle) {
        Some(code) if code != STILL_ACTIVE as u32 => {
            if std::mem::replace(ended_reported, true) {
                Vec::new()
            } else {
                vec![game_thread_ended_line(thread_id, code)]
            }
        }
        _ => match freeze_and_read(handle) {
            Ok(view) => {
                // the thread runs again here, so loader calls cannot meet a lock it held
                let sample = StallSample {
                    thread_id,
                    stalled_ms,
                    index,
                    instruction: view.instruction,
                    instruction_module: module_at(view.instruction),
                    stack: frames_in_stack_words(&view.stack_words),
                    steps: game_thread_steps(),
                };
                stall_sample_text(&sample)
                    .lines()
                    .map(str::to_owned)
                    .collect()
            }
            Err(why) => vec![stall_unreadable_line(thread_id, stalled_ms, why)],
        },
    };
    close(handle);
    lines
}

#[expect(unsafe_code, reason = "opens a handle to the game thread by its id")]
fn open_game_thread(thread_id: u32) -> Result<HANDLE, String> {
    // SAFETY: OpenThread takes plain values and returns null on failure.
    let handle = unsafe {
        OpenThread(
            THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_QUERY_LIMITED_INFORMATION,
            0,
            thread_id,
        )
    };
    if handle.is_null() {
        return Err(format!(
            "the thread {thread_id} could not be opened (it may be gone)"
        ));
    }
    Ok(handle)
}

#[expect(unsafe_code, reason = "asks whether the game thread still runs")]
fn exit_code(handle: HANDLE) -> Option<u32> {
    let mut code = 0_u32;
    // SAFETY: the handle was opened with query access; the out pointer is a local.
    (unsafe { GetExitCodeThread(handle, &mut code) } != 0).then_some(code)
}

#[expect(
    unsafe_code,
    reason = "suspends the game thread, reads its registers and copies its stack, then resumes it"
)]
fn freeze_and_read(handle: HANDLE) -> Result<FrozenView, &'static str> {
    // allocated before the thread stops: it may hold the heap lock while it is frozen
    let mut stack_words = vec![0_u64; CRASH_REPORT_STACK_SCAN_WORDS];
    // SAFETY: CONTEXT is plain data; all zeroes is a valid value before the flags are set.
    let mut context = AlignedContext(unsafe { core::mem::zeroed() });
    context.0.ContextFlags = CONTEXT_CONTROL_AMD64 | CONTEXT_INTEGER_AMD64;
    // SAFETY: the handle has suspend access; every suspend below is matched by one resume.
    if unsafe { SuspendThread(handle) } == SUSPEND_FAILED {
        return Err("the thread could not be suspended");
    }
    // SAFETY: the thread is suspended and the context is 16-byte aligned, as the call needs.
    let read = unsafe { GetThreadContext(handle, &mut context.0) } != 0;
    let mut copied = 0;
    if read {
        for (index, word) in stack_words.iter_mut().enumerate() {
            let mut done = 0_usize;
            // SAFETY: the destination is one u64 of our buffer; a failed read reports 0 and
            // leaves it untouched, and ends the copy.
            let ok = unsafe {
                ReadProcessMemory(
                    GetCurrentProcess(),
                    (context.0.Rsp + 8 * index as u64) as *const core::ffi::c_void,
                    (word as *mut u64).cast(),
                    8,
                    &mut done,
                )
            };
            if ok == 0 {
                break;
            }
            copied = index + 1;
        }
    }
    // SAFETY: matches the suspend above.
    unsafe { ResumeThread(handle) };
    if !read {
        return Err("its registers could not be read");
    }
    stack_words.truncate(copied);
    Ok(FrozenView {
        instruction: context.0.Rip,
        stack_words,
    })
}

#[expect(unsafe_code, reason = "closes the thread handle opened above")]
fn close(handle: HANDLE) {
    // SAFETY: the handle came from OpenThread and is closed once.
    unsafe { CloseHandle(handle) };
}
