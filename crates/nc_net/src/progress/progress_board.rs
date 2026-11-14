use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use crate::progress::LinkProgress;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgressEntry {
    pub at: Instant,
    pub progress: LinkProgress,
}

#[derive(Default)]
struct BoardState {
    generation: u64,
    entries: Vec<ProgressEntry>,
}

/// The progress of the session being set up, readable whole from any thread. A fresh sink starts the board
/// over; a sink handed out before that writes nothing more.
#[derive(Clone, Default)]
pub struct ProgressBoard {
    state: Arc<Mutex<BoardState>>,
}

/// The writing end of a progress board, for one session.
#[derive(Clone)]
pub struct ProgressSink {
    state: Arc<Mutex<BoardState>>,
    generation: u64,
}

impl ProgressBoard {
    pub fn fresh_sink(&self) -> ProgressSink {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.generation += 1;
        state.entries.clear();
        ProgressSink {
            state: Arc::clone(&self.state),
            generation: state.generation,
        }
    }

    /// Every entry since the last fresh sink, oldest first.
    pub fn entries(&self) -> Vec<ProgressEntry> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entries
            .clone()
    }
}

impl ProgressSink {
    pub fn send(&self, progress: LinkProgress) {
        self.send_at(Instant::now(), progress);
    }

    pub fn send_at(&self, at: Instant, progress: LinkProgress) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.generation == self.generation {
            state.entries.push(ProgressEntry { at, progress });
        }
    }
}
