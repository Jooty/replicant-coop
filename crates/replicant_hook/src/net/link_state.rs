use std::time::Instant;

use replicant_game::readiness::REPORT_STALE_MS;
use replicant_proto::{GameReadiness, PlayerSettings, SessionOptions};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReplicantLinkState {
    pub options: SessionOptions,
    pub pending_settings: Option<PlayerSettings>,
    pub local_readiness: GameReadiness,
    pub local_readiness_at: Option<Instant>,
}

impl ReplicantLinkState {
    pub fn effective_local_readiness(&self) -> GameReadiness {
        let fresh = self
            .local_readiness_at
            .is_some_and(|at| at.elapsed().as_millis() as u64 <= REPORT_STALE_MS);
        if fresh {
            self.local_readiness
        } else {
            GameReadiness::Loading
        }
    }
}
