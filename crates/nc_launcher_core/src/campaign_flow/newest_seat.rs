#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeatSource {
    Unsent,
    SessionFile,
    FromHost,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeatCandidate {
    pub source: SeatSource,
    pub play_seconds: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostStore {
    Store,
    SetAside,
}

/// The candidate with the most play time in the save itself. On a tie the earlier candidate wins,
/// so callers list the copies that exist nowhere else first.
pub fn pick_newest_seat(candidates: &[SeatCandidate]) -> Option<SeatSource> {
    let mut newest: Option<SeatCandidate> = None;
    for candidate in candidates {
        let seconds = comparable(candidate.play_seconds);
        if newest.is_none_or(|best| seconds > comparable(best.play_seconds)) {
            newest = Some(*candidate);
        }
    }
    newest.map(|candidate| candidate.source)
}

/// Whether the host replaces its stored seat with one that arrived. Only a seat with more play time
/// replaces it; any other is kept beside it.
pub fn host_store_decision(
    stored_play_seconds: Option<f64>,
    incoming_play_seconds: f64,
) -> HostStore {
    match stored_play_seconds {
        None => HostStore::Store,
        Some(stored) if comparable(incoming_play_seconds) > comparable(stored) => HostStore::Store,
        Some(_) => HostStore::SetAside,
    }
}

fn comparable(play_seconds: f64) -> f64 {
    if play_seconds.is_finite() {
        play_seconds
    } else {
        f64::NEG_INFINITY
    }
}
