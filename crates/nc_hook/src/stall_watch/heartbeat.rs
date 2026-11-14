use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Instant;

const NO_TICK_YET: u64 = u64::MAX;

static CLOCK_START: OnceLock<Instant> = OnceLock::new();
static GAME_THREAD_ID: AtomicU32 = AtomicU32::new(0);
static LAST_TICK_MS: AtomicU64 = AtomicU64::new(NO_TICK_YET);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameThreadHeartbeat {
    pub thread_id: u32,
    pub last_tick_ms: u64,
}

/// Milliseconds on the watch's own clock, which starts at its first use.
pub fn stall_clock_ms() -> u64 {
    CLOCK_START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// Called by the game thread's tick: the thread is alive and this is its id.
pub fn note_game_thread_tick(thread_id: u32) {
    GAME_THREAD_ID.store(thread_id, Ordering::Relaxed);
    LAST_TICK_MS.store(stall_clock_ms(), Ordering::Release);
}

/// The last tick the game thread made, or `None` before its first.
pub fn game_thread_heartbeat() -> Option<GameThreadHeartbeat> {
    let last_tick_ms = LAST_TICK_MS.load(Ordering::Acquire);
    (last_tick_ms != NO_TICK_YET).then(|| GameThreadHeartbeat {
        thread_id: GAME_THREAD_ID.load(Ordering::Relaxed),
        last_tick_ms,
    })
}
