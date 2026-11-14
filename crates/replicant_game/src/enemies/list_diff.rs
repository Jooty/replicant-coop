use std::collections::{BTreeMap, BTreeSet};

use replicant_proto::EnemyId;

use crate::enemies::ENEMY_LIST_DIFF_INTERVAL_MS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalKeep {
    HeldHidden,
    WaitingGrace,
    WaitingHostKill,
    HostEnded,
    HostListIncomplete,
    OutsideHostRound,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListDiff {
    pub only_host: BTreeSet<EnemyId>,
    pub only_local: BTreeMap<EnemyId, LocalKeep>,
}

#[derive(Debug, Default)]
pub struct ListDiffWatch {
    last_checked_ms: Option<u64>,
    last_logged: ListDiff,
}

impl LocalKeep {
    pub const fn text(self) -> &'static str {
        match self {
            Self::HeldHidden => "hidden and held, the host lacks it",
            Self::WaitingGrace => "not held yet, inside the grace period",
            Self::WaitingHostKill => "the host's kill of it is on its way",
            Self::HostEnded => "the host ended it, the copy is still here",
            Self::HostListIncomplete => "kept, the host's list is not complete yet",
            Self::OutsideHostRound => "kept, the host has it but left it out of its round",
        }
    }
}

impl ListDiff {
    pub fn is_empty(&self) -> bool {
        self.only_host.is_empty() && self.only_local.is_empty()
    }

    pub fn text(&self) -> String {
        let host: Vec<String> = self
            .only_host
            .iter()
            .map(|id| format!("{}:{} (no copy here)", id.group, id.ordinal))
            .collect();
        let local: Vec<String> = self
            .only_local
            .iter()
            .map(|(id, keep)| format!("{}:{} ({})", id.group, id.ordinal, keep.text()))
            .collect();
        format!(
            "lists differ, only the host has [{}], only you have [{}]",
            host.join(", "),
            local.join(", ")
        )
    }
}

impl ListDiffWatch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn observe(&mut self, now_ms: u64, diff: ListDiff) -> Option<String> {
        if self
            .last_checked_ms
            .is_some_and(|at| now_ms.saturating_sub(at) < ENEMY_LIST_DIFF_INTERVAL_MS)
        {
            return None;
        }
        self.last_checked_ms = Some(now_ms);
        if diff == self.last_logged {
            return None;
        }
        let text = if diff.is_empty() {
            "lists match again".to_owned()
        } else {
            diff.text()
        };
        self.last_logged = diff;
        Some(text)
    }
}
