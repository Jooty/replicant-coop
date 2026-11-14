use replicant_proto::{CartRide, CartRideEnd, CartRun, CartRunProgress};

use crate::carts::{
    CART_END_PULL_DELAY_MS, CART_PARTNER_NEAR_UNITS, CartSighting, CartState, parked_state_after,
    run_state_from,
};
use crate::stay_together::{JumpPartner, JumpStep, distance_between};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CartRideFacts<'a> {
    pub now_ms: u64,
    pub phase: Option<u16>,
    pub carts: &'a [CartSighting],
    pub rider_on_cart: bool,
    pub rider_position: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CartRideEvent {
    Started(CartRide),
    /// `parked` is the end the guest is told about; `None` when the cart did not park at its far
    /// end (it was removed, or the host left it mid-rail), and then nothing is sent.
    Ended {
        end: CartRideEnd,
        parked: Option<CartRide>,
        at: [f32; 3],
    },
}

/// The cart the host rides, named as its ride's start message named it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiddenCart {
    pub actor: u64,
    pub phase: u16,
    pub layout_id: Option<u16>,
    pub start_position: [f32; 3],
    pub run_state: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Ridden {
    actor: u64,
    boarded_at: [f32; 3],
    run_state: u8,
    layout_id: Option<u16>,
    start_position: [f32; 3],
    last_seen_at: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct OwedPull {
    due_ms: u64,
    from: [f32; 3],
    phase: u16,
}

/// The host's side of a cart ride: sees its own player board and start a ride, sees the ride
/// end, and a moment after the end asks for the partner to be brought along when it did not
/// arrive on its own cart.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HostRideWatch {
    phase: Option<u16>,
    last_seen: Vec<CartSighting>,
    was_on_cart: bool,
    boarded_at: Option<[f32; 3]>,
    ridden: Option<Ridden>,
    owed_pull: Option<OwedPull>,
}

impl RiddenCart {
    pub fn run(&self, seq: u32, progress: CartRunProgress) -> CartRun {
        CartRun {
            phase: self.phase,
            layout_id: self.layout_id,
            cart_position: self.start_position,
            run_state: self.run_state,
            seq,
            progress,
        }
    }
}

impl HostRideWatch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    pub fn observe(&mut self, facts: &CartRideFacts) -> Vec<CartRideEvent> {
        if facts.phase != self.phase {
            let left_mid_ride = self.ridden.map(|ridden| CartRideEvent::Ended {
                end: CartRideEnd::PhaseChanged,
                parked: None,
                at: facts.rider_position.unwrap_or(ridden.last_seen_at),
            });
            self.reset();
            self.phase = facts.phase;
            self.last_seen = facts.carts.to_vec();
            self.was_on_cart = facts.rider_on_cart;
            return left_mid_ride.into_iter().collect();
        }
        let Some(phase) = facts.phase else {
            return Vec::new();
        };
        if facts.rider_on_cart && !self.was_on_cart {
            self.boarded_at = facts.rider_position;
        }
        let flag_cleared = self.was_on_cart && !facts.rider_on_cart;
        self.was_on_cart = facts.rider_on_cart;
        let mut events = Vec::new();
        if let Some(event) = self.end_of_ride(phase, facts, flag_cleared) {
            events.push(event);
        }
        if self.ridden.is_none()
            && let Some(event) = self.start_of_ride(phase, facts)
        {
            events.push(event);
        }
        if !facts.rider_on_cart {
            self.boarded_at = None;
        }
        self.last_seen = facts.carts.to_vec();
        events
    }

    /// Due once `CART_END_PULL_DELAY_MS` after a ride ended: bring the partner over when it is in
    /// this map and more than `CART_PARTNER_NEAR_UNITS` from the host.
    pub fn pull_due(
        &mut self,
        now_ms: u64,
        position: Option<[f32; 3]>,
        partner: Option<JumpPartner>,
    ) -> JumpStep {
        let Some(owed) = self.owed_pull.filter(|owed| now_ms >= owed.due_ms) else {
            return JumpStep::Nothing;
        };
        self.owed_pull = None;
        let Some(to) = position else {
            return JumpStep::Nothing;
        };
        let Some(partner) = partner.filter(|partner| partner.phase == owed.phase) else {
            return JumpStep::Nothing;
        };
        match distance_between(partner.position, to) {
            Some(partner_away) if partner_away > CART_PARTNER_NEAR_UNITS => {
                JumpStep::BringPartner {
                    from: owed.from,
                    to,
                    partner_away,
                }
            }
            _ => JumpStep::Nothing,
        }
    }

    pub fn riding(&self) -> bool {
        self.ridden.is_some()
    }

    pub fn ridden_cart(&self) -> Option<RiddenCart> {
        let ridden = self.ridden?;
        Some(RiddenCart {
            actor: ridden.actor,
            phase: self.phase?,
            layout_id: ridden.layout_id,
            start_position: ridden.start_position,
            run_state: ridden.run_state,
        })
    }

    fn start_of_ride(&mut self, phase: u16, facts: &CartRideFacts) -> Option<CartRideEvent> {
        if !facts.rider_on_cart {
            return None;
        }
        let (before, now, from, to) = facts.carts.iter().find_map(|now| {
            let before = self.last_seen.iter().find(|seen| seen.actor == now.actor)?;
            match (before.state, now.state) {
                (CartState::Parked(from), CartState::Running(to))
                    if run_state_from(from) == Some(to) =>
                {
                    Some((*before, *now, from, to))
                }
                _ => None,
            }
        })?;
        let boarded_at = self.boarded_at.or(facts.rider_position)?;
        self.ridden = Some(Ridden {
            actor: now.actor,
            boarded_at,
            run_state: to,
            layout_id: now.layout_id,
            start_position: before.position,
            last_seen_at: now.position,
        });
        Some(CartRideEvent::Started(CartRide {
            phase,
            layout_id: now.layout_id,
            cart_position: before.position,
            from_state: from,
            to_state: to,
            boarded_at,
        }))
    }

    fn end_of_ride(
        &mut self,
        phase: u16,
        facts: &CartRideFacts,
        flag_cleared: bool,
    ) -> Option<CartRideEvent> {
        let mut ridden = self.ridden?;
        let cart = facts
            .carts
            .iter()
            .find(|cart| cart.actor == ridden.actor)
            .copied();
        if let Some(cart) = cart {
            ridden.last_seen_at = cart.position;
            self.ridden = Some(ridden);
        }
        let parked_at_end = cart.and_then(|cart| match cart.state {
            CartState::Parked(to) if parked_state_after(ridden.run_state) == Some(to) => Some(to),
            _ => None,
        });
        let end = if cart.is_none() {
            CartRideEnd::CartGone
        } else if flag_cleared {
            CartRideEnd::OnCartFlagCleared
        } else if parked_at_end.is_some() {
            CartRideEnd::CartParked
        } else if cart.is_some_and(|cart| cart.state != CartState::Running(ridden.run_state)) {
            CartRideEnd::StateLeftRun
        } else {
            return None;
        };
        self.ridden = None;
        self.owed_pull = Some(OwedPull {
            due_ms: facts.now_ms.saturating_add(CART_END_PULL_DELAY_MS),
            from: ridden.boarded_at,
            phase,
        });
        let at = facts.rider_position.unwrap_or(ridden.last_seen_at);
        let parked = parked_at_end.map(|to| CartRide {
            phase,
            layout_id: ridden.layout_id,
            cart_position: ridden.last_seen_at,
            from_state: ridden.run_state,
            to_state: to,
            boarded_at: at,
        });
        Some(CartRideEvent::Ended { end, parked, at })
    }
}
