use replicant_proto::EnemySnapshot;

use crate::enemies::ENEMY_MIRROR_STALE_MS;

#[derive(Debug, Default)]
pub struct RoundTracker {
    current: Option<Round>,
    complete: Option<CompleteRound>,
}

#[derive(Debug)]
struct Round {
    tick: u32,
    parts: u8,
    arrived: u32,
    held: u16,
    total: u16,
    round: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompleteRound {
    pub at_ms: u64,
    pub total: u16,
    pub round: u16,
}

impl RoundTracker {
    pub fn receive(&mut self, at_ms: u64, snapshot: &EnemySnapshot) {
        if let Some(current) = &self.current {
            let behind = snapshot.tick.wrapping_sub(current.tick) > u32::MAX / 2;
            if behind {
                return;
            }
        }
        if self
            .current
            .as_ref()
            .is_none_or(|current| current.tick != snapshot.tick)
        {
            self.current = Some(Round {
                tick: snapshot.tick,
                parts: snapshot.parts,
                arrived: 0,
                held: 0,
                total: snapshot.total,
                round: snapshot.round,
            });
        }
        let Some(current) = &mut self.current else {
            return;
        };
        let bit = 1_u32 << u32::from(snapshot.part);
        if current.arrived & bit == 0 {
            current.arrived |= bit;
            current.held = current
                .held
                .saturating_add(u16::try_from(snapshot.enemies.len()).unwrap_or(u16::MAX));
        }
        let every_part = (1_u32 << u32::from(current.parts)) - 1;
        if current.arrived == every_part && current.held == current.round {
            self.complete = Some(CompleteRound {
                at_ms,
                total: current.total,
                round: current.round,
            });
        }
    }

    pub fn latest_complete(&self) -> Option<CompleteRound> {
        self.complete
    }

    pub fn covers_every_host_enemy(&self, now_ms: u64) -> bool {
        self.complete.is_some_and(|round| {
            round.total == round.round
                && now_ms.saturating_sub(round.at_ms) <= ENEMY_MIRROR_STALE_MS
        })
    }
}
