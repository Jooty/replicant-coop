use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{LazyLock, Mutex, PoisonError};

use replicant_game::effects::{
    EFFECT_LOG_LINES_PER_WINDOW, EFFECT_LOG_WINDOW_MS, EffectNameTally,
    MAX_EFFECT_NAMES_IN_A_SUMMARY, RateWindow, world_effects_shown_line,
};

use crate::diagnostics::log_line;
use crate::net::net_clock_ms;

pub(super) static SENT_EFFECTS: AtomicU32 = AtomicU32::new(0);
pub(super) static SENT_SOUNDS: AtomicU32 = AtomicU32::new(0);
pub(super) static REFUSED_BY_RATE_LIMIT: AtomicU32 = AtomicU32::new(0);
pub(super) static UNREADABLE: AtomicU32 = AtomicU32::new(0);
pub(super) static SENT_STOPS: AtomicU32 = AtomicU32::new(0);
pub(super) static PLAYED_STOPS: AtomicU32 = AtomicU32::new(0);
pub(super) static SILENCED_BY_DISTANCE: AtomicU32 = AtomicU32::new(0);
pub(super) static PLAYED_EFFECTS: AtomicU32 = AtomicU32::new(0);
pub(super) static PLAYED_SOUNDS: AtomicU32 = AtomicU32::new(0);
pub(super) static PLAY_FAILED: AtomicU32 = AtomicU32::new(0);
pub(super) static DROPPED_WITHOUT_PUPPET: AtomicU32 = AtomicU32::new(0);

static WORLD_EFFECTS_SHOWN: LazyLock<Mutex<EffectNameTally>> = LazyLock::new(Mutex::default);
static LOG_WINDOW: LazyLock<Mutex<RateWindow>> = LazyLock::new(|| {
    Mutex::new(RateWindow::new(
        EFFECT_LOG_WINDOW_MS,
        EFFECT_LOG_LINES_PER_WINDOW,
    ))
});

pub(super) fn count(counter: &AtomicU32) {
    counter.fetch_add(1, Ordering::Relaxed);
}

pub(super) fn note_world_effect_shown(name: &str, shown: bool) {
    WORLD_EFFECTS_SHOWN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .note(name, shown);
}

pub(super) fn log_limited(line: &str) {
    let admitted = LOG_WINDOW
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .admit(net_clock_ms());
    if admitted {
        log_line(line);
    }
}

pub(super) fn log_summary(still_live: usize) {
    let shown = world_effects_shown_line(
        &mut WORLD_EFFECTS_SHOWN
            .lock()
            .unwrap_or_else(PoisonError::into_inner),
        MAX_EFFECT_NAMES_IN_A_SUMMARY,
    );
    if let Some(line) = shown {
        log_line(&line);
    }
    let take = |counter: &AtomicU32| counter.swap(0, Ordering::Relaxed);
    let (sent_effects, sent_sounds) = (take(&SENT_EFFECTS), take(&SENT_SOUNDS));
    let (refused, unreadable) = (take(&REFUSED_BY_RATE_LIMIT), take(&UNREADABLE));
    let (played_effects, played_sounds) = (take(&PLAYED_EFFECTS), take(&PLAYED_SOUNDS));
    let (sent_stops, played_stops) = (take(&SENT_STOPS), take(&PLAYED_STOPS));
    let (failed, dropped) = (take(&PLAY_FAILED), take(&DROPPED_WITHOUT_PUPPET));
    let silenced = take(&SILENCED_BY_DISTANCE);
    let unlogged = LOG_WINDOW
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take_refused(net_clock_ms());
    if sent_effects
        + sent_sounds
        + refused
        + unreadable
        + played_effects
        + played_sounds
        + failed
        + dropped
        + sent_stops
        + played_stops
        + silenced
        == 0
    {
        return;
    }
    log_line(&format!(
        "effects: last 5 s: sent {sent_effects} effects {sent_sounds} sounds ({refused} refused by the rate limit, {unreadable} unreadable) and {sent_stops} stops, played on the puppet {played_effects} effects {played_sounds} sounds {played_stops} stops, {silenced} sounds too far to hear ({failed} failed, {dropped} dropped with no puppet, {still_live} effects still live, {unlogged} lines not logged)"
    ));
}
