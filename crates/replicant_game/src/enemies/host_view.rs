use std::collections::BTreeSet;

use replicant_proto::EnemyId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostHas {
    Yes,
    No,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct HostListView {
    pub round_covers_host: bool,
    pub listed: Option<BTreeSet<EnemyId>>,
}

impl HostListView {
    pub fn is_known(&self) -> bool {
        self.round_covers_host || self.listed.is_some()
    }

    pub fn host_has(&self, id: EnemyId) -> HostHas {
        if self.round_covers_host {
            return HostHas::No;
        }
        match &self.listed {
            Some(listed) if listed.contains(&id) => HostHas::Yes,
            Some(_) => HostHas::No,
            None => HostHas::Unknown,
        }
    }
}
