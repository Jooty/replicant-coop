use std::collections::HashSet;

use replicant_proto::EnemyId;

/// One of the guest's own spawns inside a fight window that no host enemy is paired with yet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaitingFightSpawn {
    pub id: EnemyId,
    pub max_hp: i32,
    pub position: [f32; 3],
}

/// One of the host's live enemies (killed or removed ones left out), as the guest last sampled it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostFightEnemy {
    pub id: EnemyId,
    pub max_hp: u32,
    pub position: [f32; 3],
}

/// What the guest's waiting own spawns inside a fight window do this tick.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FightBindingPlan {
    /// Own ids that already are the host enemy they pair with.
    pub kept: Vec<EnemyId>,
    /// (own id, host id): the own enemy takes the host id no enemy here holds.
    pub renames: Vec<(EnemyId, EnemyId)>,
    /// (own id, host id): a copy here holds the host id; the own enemy takes it over and the copy goes.
    pub takeovers: Vec<(EnemyId, EnemyId)>,
    /// Own ids that pair with nothing but are a host id (live or ended): they move to an id the
    /// host never has, so no host enemy is doubled or kept alive after the host ended it.
    pub retired: Vec<EnemyId>,
}

/// Pairs each waiting own spawn with a live host enemy of its group and max hp, nearest first. An
/// id held here (not by a copy or a waiting spawn) or ever taken over is never paired again. Apply
/// the renames and retirements together, with `EnemyRoster::reassign_ids`.
pub fn plan_fight_bindings(
    waiting: &[WaitingFightSpawn],
    host_live: &[HostFightEnemy],
    host_ended: &HashSet<EnemyId>,
    held_by_others: &HashSet<EnemyId>,
    copies_here: &HashSet<EnemyId>,
    already_own: &HashSet<EnemyId>,
) -> FightBindingPlan {
    let mut pairs: Vec<(f32, usize, EnemyId)> = Vec::new();
    for (order, spawn) in waiting.iter().enumerate() {
        for host in host_live {
            let free = host.id == spawn.id
                || (!held_by_others.contains(&host.id) && !already_own.contains(&host.id));
            if host.id.group == spawn.id.group && max_hp_fits(spawn.max_hp, host.max_hp) && free {
                pairs.push((distance(spawn.position, host.position), order, host.id));
            }
        }
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut paired_spawns = HashSet::new();
    let mut paired_hosts = HashSet::new();
    let mut plan = FightBindingPlan::default();
    for (_, order, host) in pairs {
        if paired_spawns.contains(&order) || paired_hosts.contains(&host) {
            continue;
        }
        paired_spawns.insert(order);
        paired_hosts.insert(host);
        let own = waiting[order].id;
        if own == host {
            plan.kept.push(own);
        } else if copies_here.contains(&host) {
            plan.takeovers.push((own, host));
        } else {
            plan.renames.push((own, host));
        }
    }
    let host_has_id =
        |id: EnemyId| host_ended.contains(&id) || host_live.iter().any(|host| host.id == id);
    for (order, spawn) in waiting.iter().enumerate() {
        if !paired_spawns.contains(&order) && host_has_id(spawn.id) {
            plan.retired.push(spawn.id);
        }
    }
    plan.kept.sort();
    plan.renames.sort();
    plan.takeovers.sort();
    plan.retired.sort();
    plan
}

fn max_hp_fits(own: i32, host: u32) -> bool {
    // An unreadable max hp (0) on either side pairs on group and position alone.
    own <= 0 || host == 0 || own.cast_unsigned() == host
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1]).hypot(a[2] - b[2])
}
