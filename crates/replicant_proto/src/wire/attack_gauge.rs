use serde::{Deserialize, Serialize};

use crate::wire::{EnemyId, WireError};

pub const MAX_ATTACK_GAUGE_NODES: usize = 8;
pub const MAX_ATTACK_GAUGE_SECONDS: f32 = 600.0;
pub const MAX_ATTACK_GAUGE_NODE_HP: i32 = 1_000_000;
pub const MAX_ATTACK_GAUGE_HIT_DAMAGE: i32 = 1_000_000;
pub const MAX_ATTACK_GAUGE_OFFSET_ABS: f32 = 1_000.0;

/// The host's boss attack gauge in `phase`: `serial` numbers each opening, `open` is `None` once
/// the gauge has closed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttackGaugeState {
    pub phase: u16,
    pub serial: u32,
    pub open: Option<AttackGaugeOpen>,
}

/// An open gauge: the boss it sits on (and the second actor of a two-actor gauge), its clock in
/// seconds, the map points and two option bytes it was opened with, and its nodes in list order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttackGaugeOpen {
    pub boss: EnemyId,
    pub partner: Option<EnemyId>,
    pub total_seconds: f32,
    pub remaining_seconds: f32,
    pub map_points: [i32; 3],
    pub options: [u8; 2],
    pub nodes: Vec<AttackGaugeNode>,
}

/// One node of the gauge, field for field as the game's node keeps it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttackGaugeNode {
    pub bone: i32,
    pub size: f32,
    pub offset: [f32; 4],
    pub kind: i32,
    pub extra: i32,
    pub hp: i32,
    pub max_hp: i32,
}

/// The guest's game opened its own gauge on a shared boss (`open`, as the guest's game set it up)
/// while the host shows none: the guest asks the host to open the same, numbered `ask`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttackGaugeAsk {
    pub phase: u16,
    pub ask: u32,
    pub open: AttackGaugeOpen,
}

/// What one guest hit took from node `node` of the gauge opening `serial`, numbered `seq` by the
/// guest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttackGaugeHit {
    pub phase: u16,
    pub serial: u32,
    pub node: u8,
    pub damage: i32,
    pub seq: u32,
}

impl AttackGaugeState {
    pub fn validate(&self) -> Result<(), WireError> {
        self.open.as_ref().map_or(Ok(()), AttackGaugeOpen::validate)
    }
}

impl AttackGaugeAsk {
    pub fn validate(&self) -> Result<(), WireError> {
        if self.open.nodes.is_empty() {
            return Err(WireError::OutOfRange {
                field: "gauge nodes",
            });
        }
        self.open.validate()
    }
}

impl AttackGaugeOpen {
    pub fn validate(&self) -> Result<(), WireError> {
        let open = self;
        open.boss.validate()?;
        if let Some(partner) = open.partner {
            partner.validate()?;
        }
        let seconds_ok =
            |value: f32| value.is_finite() && (0.0..=MAX_ATTACK_GAUGE_SECONDS).contains(&value);
        if !seconds_ok(open.total_seconds) || !seconds_ok(open.remaining_seconds) {
            return Err(WireError::OutOfRange {
                field: "gauge seconds",
            });
        }
        if open.nodes.len() > MAX_ATTACK_GAUGE_NODES {
            return Err(WireError::OutOfRange {
                field: "gauge nodes",
            });
        }
        open.nodes.iter().try_for_each(AttackGaugeNode::validate)
    }
}

impl AttackGaugeNode {
    pub fn validate(&self) -> Result<(), WireError> {
        let floats_ok = core::iter::once(self.size)
            .chain(self.offset)
            .all(|value| value.is_finite() && value.abs() <= MAX_ATTACK_GAUGE_OFFSET_ABS);
        if !floats_ok {
            return Err(WireError::OutOfRange {
                field: "gauge node shape",
            });
        }
        let hp_ok = |value: i32| value.abs() <= MAX_ATTACK_GAUGE_NODE_HP;
        if !hp_ok(self.hp) || !hp_ok(self.max_hp) {
            return Err(WireError::OutOfRange {
                field: "gauge node hp",
            });
        }
        Ok(())
    }
}

impl AttackGaugeHit {
    pub fn validate(&self) -> Result<(), WireError> {
        if usize::from(self.node) >= MAX_ATTACK_GAUGE_NODES {
            return Err(WireError::OutOfRange {
                field: "gauge node",
            });
        }
        if !(1..=MAX_ATTACK_GAUGE_HIT_DAMAGE).contains(&self.damage) {
            return Err(WireError::OutOfRange {
                field: "gauge damage",
            });
        }
        Ok(())
    }
}
