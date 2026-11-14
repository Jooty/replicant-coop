use crate::enemies::IntervalClock;

use crate::enemies::ENEMY_COUNT_LOG_INTERVAL_MS;

#[derive(Debug, Default)]
pub struct CountLog {
    clock: IntervalClock,
    last: Option<(u16, u16, usize)>,
    last_host_overflow: Option<(u16, usize)>,
}

impl CountLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn guest_line(
        &mut self,
        now_ms: u64,
        host_total: u16,
        round: u16,
        mirror_len: usize,
        local_count: usize,
    ) -> Option<String> {
        let numbers = (host_total, round, mirror_len);
        if self.last == Some(numbers) {
            return None;
        }
        if !self.clock.due(now_ms, ENEMY_COUNT_LOG_INTERVAL_MS) {
            return None;
        }
        let differ = host_total != round || usize::from(host_total) != mirror_len;
        let was_differing = self
            .last
            .is_some_and(|(total, round, len)| total != round || usize::from(total) != len);
        self.last = Some(numbers);
        if differ {
            Some(format!(
                "counts differ: the host has {host_total}, its round holds {round}, your mirror holds {mirror_len}, you have {local_count}"
            ))
        } else if was_differing {
            Some(format!(
                "counts agree again: the host has {host_total}, its round holds {round}, your mirror holds {mirror_len}"
            ))
        } else {
            None
        }
    }

    pub fn host_line(&mut self, host_total: usize, round_capacity: usize) -> Option<String> {
        let overflow = (host_total > round_capacity).then_some((
            u16::try_from(host_total).unwrap_or(u16::MAX),
            round_capacity,
        ));
        if overflow == self.last_host_overflow {
            return None;
        }
        self.last_host_overflow = overflow;
        overflow.map(|(total, capacity)| {
            format!(
                "the host has {total} enemies and a round holds {capacity}: the farthest are left out of the round and only listed by id"
            )
        })
    }
}
