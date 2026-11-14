use std::sync::atomic::{AtomicU64, Ordering};

use crate::net::net_clock_ms;

const NEVER_MOVED: u64 = u64::MAX;
const MOD_MOVE_COUNTS_FOR_MS: u64 = 400;

static LAST_MOD_MOVE_AT_MS: AtomicU64 = AtomicU64::new(NEVER_MOVED);

pub(crate) fn note_mod_move() {
    LAST_MOD_MOVE_AT_MS.store(net_clock_ms(), Ordering::Release);
}

pub(crate) fn mod_moved_the_player_just_now() -> bool {
    let at = LAST_MOD_MOVE_AT_MS.load(Ordering::Acquire);
    at != NEVER_MOVED && net_clock_ms().saturating_sub(at) < MOD_MOVE_COUNTS_FOR_MS
}
