use crate::enemy_bullets::EnemyBullet;

/// The owner's map from a bullet's actor to the instance its flights carry, for the last two
/// scans: a bullet that made its impact and ended before this scan is still named.
#[derive(Debug, Clone, Default)]
pub struct ShotInstances {
    current: Vec<(u64, u16)>,
    previous: Vec<(u64, u16)>,
}

impl ShotInstances {
    pub fn note_scan(&mut self, bullets: &[EnemyBullet]) {
        self.previous = core::mem::replace(
            &mut self.current,
            bullets
                .iter()
                .map(|seen| (seen.bullet.actor, seen.bullet.slot))
                .collect(),
        );
    }

    pub fn instance_of(&self, actor: u64) -> Option<u16> {
        self.current
            .iter()
            .chain(&self.previous)
            .find(|(known, _)| *known == actor)
            .map(|(_, instance)| *instance)
    }
}
