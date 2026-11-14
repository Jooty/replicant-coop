use crate::enemies::HOST_HIT_STAGGER_CHECK_WINDOW_MS;
use crate::memory::GameMemory;
use crate::move_log::{CURRENT_STATE_POINTER_OFFSET, state_name_for_rva};

pub const DAMAGE_STATE_NAME: &str = "StateActionDamage";

/// One host hit applied to the guest's body, waiting to see whether its body enters the damage
/// state within the check window.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingStaggerCheck {
    pub attacker: String,
    pub kind: u8,
    pub push: f32,
    pub applied_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StaggerVerdict {
    Entered {
        check: PendingStaggerCheck,
        after_ms: u64,
    },
    NotEntered {
        check: PendingStaggerCheck,
        last_state: &'static str,
    },
}

/// Watches the guest's body after a host hit that asked for a reaction: one check at a time,
/// so a burst of hits gives one line per damage-state entry or miss.
#[derive(Debug, Clone, Default)]
pub struct HostHitStaggerWatch {
    pending: Option<PendingStaggerCheck>,
}

impl HostHitStaggerWatch {
    pub const fn new() -> Self {
        Self { pending: None }
    }

    /// A host hit was applied. `already_in_damage_state` is the body's state just before it: a
    /// body already reeling proves nothing about this hit, so no check starts.
    pub fn hit_applied(
        &mut self,
        attacker: &str,
        kind: u8,
        push: f32,
        already_in_damage_state: bool,
        now_ms: u64,
    ) {
        if kind == 0 || already_in_damage_state || self.pending.is_some() {
            return;
        }
        self.pending = Some(PendingStaggerCheck {
            attacker: attacker.to_owned(),
            kind,
            push,
            applied_at_ms: now_ms,
        });
    }

    /// Once a tick with the body's current state name.
    pub fn observe(&mut self, state: &'static str, now_ms: u64) -> Option<StaggerVerdict> {
        let check = self.pending.as_ref()?;
        let after_ms = now_ms.saturating_sub(check.applied_at_ms);
        if state == DAMAGE_STATE_NAME {
            let check = self.pending.take()?;
            return Some(StaggerVerdict::Entered { check, after_ms });
        }
        if after_ms < HOST_HIT_STAGGER_CHECK_WINDOW_MS {
            return None;
        }
        let check = self.pending.take()?;
        Some(StaggerVerdict::NotEntered {
            check,
            last_state: state,
        })
    }
}

/// The damage-state motion the player's game starts for a reaction kind, before guard and
/// parry results: kinds 1 to 3 reel, 4 is blown away, 5 is thrown high.
pub fn player_reaction_motion(kind: u8) -> &'static str {
    match kind {
        0 => "no reaction",
        1 => "damage_reel_s",
        2 => "damage_reel_m",
        3 => "damage_reel_l",
        4 => "damage_blow",
        5 => "damage_hblow",
        _ => "unknown",
    }
}

/// The name of the action state the player actor at `actor` is in, "unknown" when unreadable.
pub fn read_player_state_name<M: GameMemory>(memory: &M, actor: u64) -> &'static str {
    memory
        .read_u64(actor + CURRENT_STATE_POINTER_OFFSET)
        .ok()
        .filter(|state| *state != 0)
        .and_then(|state| memory.read_u64(state).ok())
        .and_then(|vftable| u32::try_from(vftable.checked_sub(memory.module_base())?).ok())
        .map_or("unknown", state_name_for_rva)
}
