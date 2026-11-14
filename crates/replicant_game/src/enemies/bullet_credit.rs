use replicant_proto::EnemyId;

/// The enemy a hit counts for, and its actor: the record's attacker or owner when either is an
/// enemy, else the enemy that fired the bullet named as the attacker (a bullet's record names no
/// owner).
pub fn credited_enemy(
    sources: [Option<u64>; 2],
    bullet_owner: Option<u64>,
    enemy_of: impl Fn(u64) -> Option<EnemyId>,
) -> Option<(u64, EnemyId)> {
    sources
        .into_iter()
        .chain([bullet_owner])
        .flatten()
        .find_map(|actor| enemy_of(actor).map(|id| (actor, id)))
}
