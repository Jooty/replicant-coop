use replicant_proto::MAX_SHOT_SCALE_GROWTH_PER_SECOND;

use crate::memory::GameMemory;
use crate::offsets::{
    BULLET_HIT_SPHERE_RADIUS_OFFSET, NORMAL_BULLET_ATTACK_SETTINGS_POINTER_OFFSET,
    NORMAL_BULLET_HIT_RADIUS_GROWTH_RATE_OFFSET,
    NORMAL_BULLET_SETTINGS_EFFECT_SCALE_DIVISOR_OFFSET,
};

const SMALLEST_RADIUS_TO_GROW_FROM: f32 = 0.01;

/// How fast an ordinary bullet's effects grow, as a share of their scale now, per second. The
/// game draws a widening bullet's effects at a scale in proportion to its hit radius, and the
/// radius grows by a fixed amount each second.
pub fn shot_scale_growth<M: GameMemory>(memory: &M, bullet: u64) -> f32 {
    let Ok(settings) = memory.read_u64(bullet + NORMAL_BULLET_ATTACK_SETTINGS_POINTER_OFFSET)
    else {
        return 0.0;
    };
    let effects_scale_with_radius = settings != 0
        && memory
            .read_f32(settings + NORMAL_BULLET_SETTINGS_EFFECT_SCALE_DIVISOR_OFFSET)
            .is_ok_and(|divisor| divisor > 0.0);
    if !effects_scale_with_radius {
        return 0.0;
    }
    let (Ok(radius), Ok(growth)) = (
        memory.read_f32(bullet + BULLET_HIT_SPHERE_RADIUS_OFFSET),
        memory.read_f32(bullet + NORMAL_BULLET_HIT_RADIUS_GROWTH_RATE_OFFSET),
    ) else {
        return 0.0;
    };
    scale_growth_of_radius(radius, growth)
}

/// The share of its size a radius of `radius` grows by in a second at `growth_per_second`, or
/// 0 for a radius too small to grow from, or one that does not grow.
pub fn scale_growth_of_radius(radius: f32, growth_per_second: f32) -> f32 {
    if radius.is_nan()
        || growth_per_second.is_nan()
        || radius < SMALLEST_RADIUS_TO_GROW_FROM
        || growth_per_second <= 0.0
    {
        return 0.0;
    }
    (growth_per_second / radius).min(MAX_SHOT_SCALE_GROWTH_PER_SECOND)
}
