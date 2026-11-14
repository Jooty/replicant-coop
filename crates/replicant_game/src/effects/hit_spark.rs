use std::collections::HashSet;

use crate::offsets::BULLET_CREATE_EFFECTS_RVA_RANGE;

#[derive(Debug, Default)]
pub struct HitSparkNames {
    seen: HashSet<String>,
}

impl HitSparkNames {
    pub fn is_first(&mut self, name: &str) -> bool {
        self.seen.insert(name.to_owned())
    }
}

pub fn spawn_is_sent(outermost: bool, local_hit_in_flight: bool) -> bool {
    outermost || local_hit_in_flight
}

/// True for an effect a shot's own creation makes from its effect slots: the shot sync already
/// draws those on the partner, so sending the spawn as well would draw it twice.
pub fn spawn_is_drawn_by_shot_sync(create_by_id_caller_rva: u32) -> bool {
    BULLET_CREATE_EFFECTS_RVA_RANGE.contains(&create_by_id_caller_rva)
}

pub fn hit_spark_log_line(name: &str, via: &str) -> String {
    format!("effects: hit spark {name} via {via}")
}
