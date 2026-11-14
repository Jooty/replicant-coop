use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use replicant_game::new_game::{TITLE_HOLD_TIMEOUT_MS, TitleHold, TitleHoldInputs, title_hold};

use crate::diagnostics::log_line;
use crate::event_log::post_event;
use crate::net::{self, NetSharedExt, NetState};

static HOLD_STARTED: OnceLock<Instant> = OnceLock::new();
static LINKED_LAUNCH: AtomicBool = AtomicBool::new(false);
static HOLD_ANNOUNCED: AtomicBool = AtomicBool::new(false);

pub(super) fn start_hold_clock(linked_launch: bool) {
    LINKED_LAUNCH.store(linked_launch, Ordering::Release);
    let _ = HOLD_STARTED.set(Instant::now());
}

pub(super) fn hold_for_partner() -> bool {
    let started = *HOLD_STARTED.get_or_init(Instant::now);
    let waited_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let Some(net) = net::handle() else {
        let waiting = LINKED_LAUNCH.load(Ordering::Acquire) && waited_ms < TITLE_HOLD_TIMEOUT_MS;
        if waiting && !HOLD_ANNOUNCED.swap(true, Ordering::Relaxed) {
            log_line("boot: title held until the link to the launcher is up");
        }
        return waiting;
    };
    let view = net.view();
    let decision = title_hold(TitleHoldInputs {
        waited_ms,
        launcher_linked: view.state != NetState::WaitingForLauncher,
        partner_readiness: view.partner_watch().readiness(),
    });
    match decision {
        TitleHold::Hold => {
            if !HOLD_ANNOUNCED.swap(true, Ordering::Relaxed) {
                log_line("boot: title held until the partner's game is open");
                let partner = view
                    .partner_name()
                    .map_or_else(|| "your partner".to_owned(), str::to_owned);
                post_event(&format!("Waiting for {partner}'s game"));
            }
            true
        }
        TitleHold::Release => {
            if HOLD_ANNOUNCED.load(Ordering::Relaxed) {
                log_line(&format!("boot: title released after {waited_ms} ms"));
            }
            false
        }
        TitleHold::TimedOut => {
            log_line(&format!(
                "boot: title hold timed out after {waited_ms} ms, starting alone"
            ));
            false
        }
    }
}
