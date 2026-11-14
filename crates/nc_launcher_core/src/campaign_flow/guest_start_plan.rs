use crate::campaign_flow::{SeatCandidate, SeatSource, pick_newest_seat};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuestStartPlan {
    pub boot_from: SeatSource,
    pub send_back: bool,
}

/// What the guest boots when the host's seat arrives: the newest of the host's copy, a copy this
/// machine could not deliver last time, and the session file already on disk. A local copy that
/// wins goes back to the host.
pub fn plan_guest_start(
    from_host_play_seconds: f64,
    unsent_play_seconds: Option<f64>,
    session_file_play_seconds: Option<f64>,
) -> GuestStartPlan {
    let mut candidates = Vec::new();
    if let Some(play_seconds) = unsent_play_seconds {
        candidates.push(SeatCandidate {
            source: SeatSource::Unsent,
            play_seconds,
        });
    }
    if let Some(play_seconds) = session_file_play_seconds {
        candidates.push(SeatCandidate {
            source: SeatSource::SessionFile,
            play_seconds,
        });
    }
    candidates.push(SeatCandidate {
        source: SeatSource::FromHost,
        play_seconds: from_host_play_seconds,
    });
    let boot_from = pick_newest_seat(&candidates).unwrap_or(SeatSource::FromHost);
    GuestStartPlan {
        boot_from,
        send_back: boot_from != SeatSource::FromHost,
    }
}
