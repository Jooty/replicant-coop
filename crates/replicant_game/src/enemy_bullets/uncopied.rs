use std::collections::{BTreeMap, BTreeSet};

use crate::enemy_bullets::{UncopiedScriptBullet, UncopiedWhy};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct UncopiedKind {
    group: u16,
    ordinal: u16,
    why: UncopiedWhy,
    laser: bool,
}

/// The host's record of script bullets it could not copy: the most of each kind at once in a
/// summary window, and which kinds it has already named in the log.
#[derive(Debug, Clone, Default)]
pub struct UncopiedTally {
    most_at_once: BTreeMap<UncopiedKind, u32>,
    named: BTreeSet<UncopiedKind>,
}

impl UncopiedTally {
    /// Counts one scan; returns a line for each kind seen for the first time.
    pub fn add_scan(&mut self, uncopied: &[UncopiedScriptBullet]) -> Vec<String> {
        let mut this_scan: BTreeMap<UncopiedKind, u32> = BTreeMap::new();
        for bullet in uncopied {
            let kind = UncopiedKind {
                group: bullet.enemy.group,
                ordinal: bullet.enemy.ordinal,
                why: bullet.why,
                laser: bullet.laser,
            };
            *this_scan.entry(kind).or_default() += 1;
        }
        let mut first_lines = Vec::new();
        for (kind, count) in this_scan {
            let most = self.most_at_once.entry(kind).or_default();
            *most = (*most).max(count);
            if self.named.insert(kind) {
                first_lines.push(format!(
                    "enemy bullets: first script bullet with no effect to copy from enemy {}:{}: {}{}",
                    kind.group,
                    kind.ordinal,
                    uncopied_why_text(kind.why),
                    laser_text(kind.laser)
                ));
            }
        }
        first_lines
    }

    /// The summary of the window that ends now, if any bullet was left uncopied in it.
    pub fn take_summary(&mut self) -> Option<String> {
        let most = core::mem::take(&mut self.most_at_once);
        if most.is_empty() {
            return None;
        }
        let parts: Vec<String> = most
            .iter()
            .map(|(kind, count)| {
                format!(
                    "{count} from {}:{} {}{}",
                    kind.group,
                    kind.ordinal,
                    uncopied_why_text(kind.why),
                    laser_text(kind.laser)
                )
            })
            .collect();
        Some(format!(
            "enemy bullets: last 5 s script bullets with no effect to copy, most at once: {}",
            parts.join(", ")
        ))
    }
}

pub fn uncopied_why_text(why: UncopiedWhy) -> String {
    match why {
        UncopiedWhy::NoEffectObject => "with no effect object".to_owned(),
        UncopiedWhy::UnknownEffectObject { vtable_rva } => {
            format!("with an effect object of class {vtable_rva:#x}")
        }
        UncopiedWhy::EffectIdOutOfRange { effect_id } => {
            format!("with effect id {effect_id}, which the wire does not carry")
        }
        UncopiedWhy::EffectGone { effect_id } => {
            format!("with effect id {effect_id} whose effect is gone")
        }
    }
}

fn laser_text(laser: bool) -> &'static str {
    if laser { ", spawned as a laser" } else { "" }
}
