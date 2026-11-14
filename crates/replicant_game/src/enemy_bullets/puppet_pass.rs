use crate::enemy_bullets::PUPPET_BULLET_REACH_MARGIN;

/// What the host knows about one bullet in the frame it submits its attacks, to decide whether
/// the bullet runs its attacks a second time with the guest's copy as its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PuppetPassInputs {
    /// A partner is in the host's phase, is not downed and its copy exists here.
    pub puppet_takes_bullets: bool,
    pub in_second_pass: bool,
    pub deals_damage: bool,
    pub aims_at_puppet: bool,
    pub within_reach: bool,
    pub fired_by_enemy: bool,
}

/// True when a bullet's attacks should be submitted once more with the guest's copy as the
/// target hint, so the game's own test can find the copy.
pub fn needs_puppet_pass(inputs: PuppetPassInputs) -> bool {
    inputs.puppet_takes_bullets
        && !inputs.in_second_pass
        && inputs.deals_damage
        && !inputs.aims_at_puppet
        && inputs.within_reach
        && inputs.fired_by_enemy
}

/// A script bullet submits attacks only with a target group and either a hit sphere or a beam.
pub fn script_bullet_deals_damage(target_group: u32, hit_radius: f32, beam: bool) -> bool {
    target_group != 0 && (hit_radius > 0.0 || beam)
}

/// True when the guest's copy is close enough to the bullet, or to the beam between the bullet
/// and `beam_end`, that the game's test could touch it.
pub fn bullet_reaches_puppet(
    puppet: [f32; 3],
    bullet: [f32; 3],
    beam_end: Option<[f32; 3]>,
    hit_radius: f32,
) -> bool {
    let reach = hit_radius.max(0.0) + PUPPET_BULLET_REACH_MARGIN;
    let nearest = beam_end.map_or(bullet, |end| nearest_on_segment(puppet, bullet, end));
    let squared: f32 = (0..3)
        .map(|axis| (puppet[axis] - nearest[axis]).powi(2))
        .sum();
    squared <= reach * reach
}

fn nearest_on_segment(point: [f32; 3], start: [f32; 3], end: [f32; 3]) -> [f32; 3] {
    let along = [end[0] - start[0], end[1] - start[1], end[2] - start[2]];
    let length_squared: f32 = along.iter().map(|axis| axis * axis).sum();
    if length_squared <= f32::EPSILON {
        return start;
    }
    let dot: f32 = (0..3)
        .map(|axis| (point[axis] - start[axis]) * along[axis])
        .sum();
    let t = (dot / length_squared).clamp(0.0, 1.0);
    [
        start[0] + along[0] * t,
        start[1] + along[1] * t,
        start[2] + along[2] * t,
    ]
}
