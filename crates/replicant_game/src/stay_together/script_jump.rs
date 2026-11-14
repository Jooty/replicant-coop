use crate::stay_together::{
    SCRIPT_JUMP_MIN_UNITS, SCRIPT_JUMP_PARTNER_NEAR_UNITS, SCRIPT_JUMP_SETTLE_MS, distance_between,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JumpPartner {
    pub position: [f32; 3],
    pub phase: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JumpFacts {
    pub now_ms: u64,
    pub position: Option<[f32; 3]>,
    pub phase: Option<u16>,
    pub script_pause_in_force: bool,
    pub mod_moved_the_host: bool,
    pub partner: Option<JumpPartner>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JumpStep {
    Nothing,
    BringPartner {
        from: [f32; 3],
        to: [f32; 3],
        partner_away: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SettlingJump {
    from: [f32; 3],
    last_jump_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ScriptJumpWatch {
    last_position: Option<[f32; 3]>,
    last_phase: Option<u16>,
    settling: Option<SettlingJump>,
}

impl ScriptJumpWatch {
    pub const fn new() -> Self {
        Self {
            last_position: None,
            last_phase: None,
            settling: None,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    pub fn observe(&mut self, facts: &JumpFacts) -> JumpStep {
        let before = self.last_position;
        let phase_before = self.last_phase;
        self.last_position = facts.position;
        self.last_phase = facts.phase;
        let (Some(position), Some(phase)) = (facts.position, facts.phase) else {
            self.settling = None;
            return JumpStep::Nothing;
        };
        if phase_before != Some(phase) || facts.mod_moved_the_host {
            self.settling = None;
            return JumpStep::Nothing;
        }
        let moved = before
            .and_then(|from| distance_between(from, position))
            .unwrap_or(0.0);
        if facts.script_pause_in_force && moved >= SCRIPT_JUMP_MIN_UNITS {
            let from = self.settling.map_or(before, |jump| Some(jump.from));
            self.settling = from.map(|from| SettlingJump {
                from,
                last_jump_ms: facts.now_ms,
            });
            return JumpStep::Nothing;
        }
        let Some(jump) = self.settling else {
            return JumpStep::Nothing;
        };
        if facts.now_ms.saturating_sub(jump.last_jump_ms) < SCRIPT_JUMP_SETTLE_MS {
            return JumpStep::Nothing;
        }
        self.settling = None;
        bring_a_far_partner(jump.from, position, phase, facts.partner)
    }
}

fn bring_a_far_partner(
    from: [f32; 3],
    to: [f32; 3],
    phase: u16,
    partner: Option<JumpPartner>,
) -> JumpStep {
    let Some(partner) = partner.filter(|partner| partner.phase == phase) else {
        return JumpStep::Nothing;
    };
    match distance_between(partner.position, to) {
        Some(partner_away) if partner_away > SCRIPT_JUMP_PARTNER_NEAR_UNITS => {
            JumpStep::BringPartner {
                from,
                to,
                partner_away,
            }
        }
        _ => JumpStep::Nothing,
    }
}
