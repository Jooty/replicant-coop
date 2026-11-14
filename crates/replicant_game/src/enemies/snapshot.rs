use std::f32::consts::{PI, TAU};

use replicant_proto::{
    EnemyId, EnemySnapshot, EnemyState, MAX_ENEMIES_PER_SNAPSHOT, MAX_ENEMY_SNAPSHOT_PARTS, MAX_HP,
    MAX_MOTION_FRAME, MAX_MOTION_SPEED_ABS, MAX_POSITION_ABS, MotionState, NO_MOTION_NAME_HASH,
};

use crate::enemies::EnemySighting;
use crate::puppet::LayerMotion;

pub const ENEMY_ROUND_CAPACITY: usize =
    MAX_ENEMIES_PER_SNAPSHOT * MAX_ENEMY_SNAPSHOT_PARTS as usize;

/// The wire form of one sighting, or `None` when its position is not a usable number.
pub fn enemy_state(id: EnemyId, sighting: &EnemySighting) -> Option<EnemyState> {
    if sighting
        .position
        .iter()
        .any(|axis| !axis.is_finite() || axis.abs() > MAX_POSITION_ABS)
    {
        return None;
    }
    let max_hp = u32::try_from(sighting.max_hp).unwrap_or(0).min(MAX_HP);
    let hp = u32::try_from(sighting.hp).unwrap_or(0).min(max_hp);
    let yaw_radians = if sighting.yaw_radians.is_finite() {
        (sighting.yaw_radians + PI).rem_euclid(TAU) - PI
    } else {
        0.0
    };
    Some(EnemyState {
        id,
        position: sighting.position,
        yaw_radians,
        motion: sighting.motion.and_then(motion_state),
        hp,
        max_hp,
    })
}

/// Splits one snapshot into message-sized parts. When there are more enemies than the parts can
/// hold, the ones nearest to any player go first and the farthest are left out.
pub fn split_snapshot(
    tick: u32,
    phase: u16,
    mut enemies: Vec<EnemyState>,
    players: &[[f32; 3]],
) -> Vec<EnemySnapshot> {
    let total = u16::try_from(enemies.len()).unwrap_or(u16::MAX);
    enemies.sort_by(|a, b| {
        nearest_player_distance(a.position, players)
            .total_cmp(&nearest_player_distance(b.position, players))
            .then(a.id.cmp(&b.id))
    });
    enemies.truncate(ENEMY_ROUND_CAPACITY);
    let round = u16::try_from(enemies.len()).unwrap_or(u16::MAX);
    let chunks: Vec<Vec<EnemyState>> = if enemies.is_empty() {
        vec![Vec::new()]
    } else {
        enemies
            .chunks(MAX_ENEMIES_PER_SNAPSHOT)
            .map(<[EnemyState]>::to_vec)
            .collect()
    };
    let parts = u8::try_from(chunks.len()).unwrap_or(MAX_ENEMY_SNAPSHOT_PARTS);
    (0_u8..)
        .zip(chunks)
        .map(|(part, enemies)| EnemySnapshot {
            tick,
            phase,
            part,
            parts,
            total,
            round,
            enemies,
        })
        .collect()
}

fn nearest_player_distance(position: [f32; 3], players: &[[f32; 3]]) -> f32 {
    players
        .iter()
        .map(|player| {
            position
                .iter()
                .zip(player)
                .map(|(a, b)| (a - b) * (a - b))
                .sum::<f32>()
        })
        .filter(|squared| squared.is_finite())
        .fold(f32::INFINITY, f32::min)
}

fn motion_state(motion: LayerMotion) -> Option<MotionState> {
    if !motion.frame.is_finite() || !motion.speed.is_finite() {
        return None;
    }
    Some(MotionState {
        index: u16::try_from(motion.index).ok()?,
        frame: motion.frame.clamp(0.0, MAX_MOTION_FRAME),
        speed: motion
            .speed
            .clamp(-MAX_MOTION_SPEED_ABS, MAX_MOTION_SPEED_ABS),
        looping: motion.looping,
        name_hash: NO_MOTION_NAME_HASH,
    })
}
