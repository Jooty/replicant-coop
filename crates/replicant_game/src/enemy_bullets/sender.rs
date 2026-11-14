use std::collections::VecDeque;

use replicant_proto::{BulletFlight, BulletLook, EnemyBulletFlight, EnemyId};

use crate::bullet_flights::{BULLET_LOOK_REPEAT_EVERY_MESSAGES, BulletTracker, SightedBullet};
use crate::enemy_bullets::{
    EnemyBullet, MAX_ENEMY_BULLET_MOVES_SENT_PER_SCAN, look_resized, look_scale,
};

#[derive(Debug, Clone, Copy)]
struct SentBullet {
    slot: u16,
    messages: u32,
    look_sent: Option<BulletLook>,
}

/// The host's side: turns each scan of enemy bullets into the flights to send, each tagged with
/// its enemy. A scan sends at most `MAX_ENEMY_BULLET_MOVES_SENT_PER_SCAN` moves, new bullets
/// first, then the ones that waited longest, and one end per bullet the guest was told of. A move
/// carries the look every few messages, and on every message while the bullet's effects change size.
#[derive(Debug, Default)]
pub struct EnemyBulletSender {
    tracker: BulletTracker,
    owners: Vec<(u16, EnemyId)>,
    sent_longest_ago_first: VecDeque<SentBullet>,
    resized: Vec<(u16, f32)>,
    started: Vec<u16>,
}

impl EnemyBulletSender {
    pub fn update(&mut self, sighted: &[EnemyBullet], now_ms: u64) -> Vec<EnemyBulletFlight> {
        let bullets: Vec<SightedBullet> = sighted.iter().map(|seen| seen.bullet).collect();
        let previous = core::mem::replace(
            &mut self.owners,
            sighted
                .iter()
                .map(|seen| (seen.bullet.slot, seen.enemy))
                .collect(),
        );
        let (ended, moving): (Vec<BulletFlight>, Vec<BulletFlight>) = self
            .tracker
            .update(&bullets, now_ms)
            .into_iter()
            .partition(|flight| flight.ended);
        // An ended slot may already hold a new bullet, so its owner is the old one.
        let mut out = self.ends_the_guest_knows(ended, &previous);
        self.resized.clear();
        self.started.clear();
        for slot in self.slots_due(&moving) {
            let (Some(flight), Some(seen)) = (
                moving.iter().find(|flight| flight.instance == slot),
                sighted.iter().find(|seen| seen.bullet.slot == slot),
            ) else {
                continue;
            };
            let sent = self.take_sent(slot);
            if sent.is_none() {
                self.started.push(slot);
            }
            let messages = sent.map_or(0, |sent| sent.messages);
            let look_sent = sent.and_then(|sent| sent.look_sent);
            let resized = look_sent.is_some_and(|before| look_resized(&before, &seen.bullet.look));
            let with_look = seen.sprite.is_none()
                && (messages.is_multiple_of(BULLET_LOOK_REPEAT_EVERY_MESSAGES) || resized);
            let look = with_look.then_some(seen.bullet.look);
            if resized && with_look {
                self.resized.push((slot, look_scale(&seen.bullet.look)));
            }
            self.sent_longest_ago_first.push_back(SentBullet {
                slot,
                messages: messages.wrapping_add(1),
                look_sent: look.or(look_sent),
            });
            out.push(EnemyBulletFlight {
                enemy: seen.enemy,
                flight: BulletFlight { look, ..*flight },
                sprite: seen.sprite,
                scale_growth_per_second: seen.scale_growth_per_second,
            });
        }
        out
    }

    pub fn end_all(&mut self) -> Vec<EnemyBulletFlight> {
        let owners = core::mem::take(&mut self.owners);
        let ended = self.tracker.end_all();
        self.ends_the_guest_knows(ended, &owners)
    }

    /// The bullets whose look the last `update` sent because their effects changed size, with
    /// the largest scale each is drawn at now.
    pub fn resized_in_last_update(&self) -> &[(u16, f32)] {
        &self.resized
    }

    /// The bullets whose first message the last `update` sent.
    pub fn started_in_last_update(&self) -> &[u16] {
        &self.started
    }

    pub fn len(&self) -> usize {
        self.tracker.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tracker.is_empty()
    }

    fn ends_the_guest_knows(
        &mut self,
        ended: Vec<BulletFlight>,
        owners: &[(u16, EnemyId)],
    ) -> Vec<EnemyBulletFlight> {
        ended
            .into_iter()
            .filter_map(|flight| {
                self.take_sent(flight.instance)?;
                owner_of(owners, flight.instance).map(|enemy| EnemyBulletFlight {
                    enemy,
                    flight,
                    sprite: None,
                    scale_growth_per_second: 0.0,
                })
            })
            .collect()
    }

    fn slots_due(&self, moving: &[BulletFlight]) -> Vec<u16> {
        let is_moving = |slot: u16| moving.iter().any(|flight| flight.instance == slot);
        let never_sent = moving.iter().map(|flight| flight.instance).filter(|slot| {
            !self
                .sent_longest_ago_first
                .iter()
                .any(|sent| sent.slot == *slot)
        });
        let waiting = self
            .sent_longest_ago_first
            .iter()
            .map(|sent| sent.slot)
            .filter(|slot| is_moving(*slot));
        never_sent
            .chain(waiting)
            .take(MAX_ENEMY_BULLET_MOVES_SENT_PER_SCAN)
            .collect()
    }

    fn take_sent(&mut self, slot: u16) -> Option<SentBullet> {
        let index = self
            .sent_longest_ago_first
            .iter()
            .position(|sent| sent.slot == slot)?;
        self.sent_longest_ago_first.remove(index)
    }
}

fn owner_of(owners: &[(u16, EnemyId)], slot: u16) -> Option<EnemyId> {
    owners
        .iter()
        .find(|(known, _)| *known == slot)
        .map(|(_, enemy)| *enemy)
}
