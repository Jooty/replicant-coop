use replicant_proto::{CartRide, CartRun};

use crate::carts::{
    CART_BOARD_CONFIRM_MS, CART_BOARD_GIVE_UP_MS, CART_BOARD_SETTLE_MS, CART_BOARD_TRIES,
    CART_END_GRACE_MS, CART_OWN_RIDE_LONGEST_MS, CART_RIDER_LIFT_UNITS, CartRideEnd, CartSighting,
    CartState, cart_state, match_cart, match_ride_cart,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OwnCartFacts<'a> {
    pub now_ms: u64,
    pub phase: Option<u16>,
    pub carts: &'a [CartSighting],
    pub may_place_rider: bool,
    pub rider_on_cart: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CartFollowStep {
    Nothing,
    /// Put this game's cart at the end the host's ride starts from.
    Park {
        actor: u64,
        state: u8,
    },
    /// Stand this game's player where the host stood when it boarded.
    PlaceRider {
        actor: u64,
        spot: [f32; 3],
    },
    /// Ask this game's cart to take its player aboard, as the game's action button does
    /// (`CActorTruck` message 0x11); the game itself sets the on-cart flag and starts the run.
    Board {
        actor: u64,
        try_number: u8,
    },
    /// The game took this player aboard: its on-cart flag is set.
    Boarded {
        actor: u64,
    },
    /// The game refused every boarding asked for (the on-cart flag stayed clear).
    BoardRefused {
        tries: u8,
    },
    /// This player's own ride ended, by the game's end message or with its cart removed.
    OwnRideEnded {
        actor: u64,
        end: CartRideEnd,
    },
    /// The host's ride ended and this game's cart is not at that end: put it there.
    ParkAtEnd {
        actor: u64,
        state: u8,
    },
    NoCartHere {
        ride: CartRide,
    },
    GaveUpBoarding {
        waited_ms: u64,
    },
    OtherMap,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Boarding {
    ride: CartRide,
    arrived_ms: u64,
    placed_ms: Option<u64>,
    asked_ms: Option<u64>,
    tries: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Follow {
    Idle,
    Boarding(Boarding),
    Riding {
        actor: u64,
        phase: u16,
        layout_id: Option<u16>,
        run_state: u8,
    },
    Ending {
        ride: CartRide,
        riding: Option<u64>,
        due_ms: u64,
        give_up_ms: u64,
    },
}

/// The guest's side of a cart ride the host started: its own cart, parked where the host's was,
/// takes its own player aboard by the game's own boarding, and ends where the host's ended.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuestCartFollow {
    follow: Follow,
}

impl Default for GuestCartFollow {
    fn default() -> Self {
        Self::new()
    }
}

impl GuestCartFollow {
    pub const fn new() -> Self {
        Self {
            follow: Follow::Idle,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    pub fn host_rode(&mut self, ride: CartRide, now_ms: u64) {
        if let CartState::Running(_) = cart_state(i32::from(ride.to_state)) {
            self.follow = Follow::Boarding(Boarding {
                ride,
                arrived_ms: now_ms,
                placed_ms: None,
                asked_ms: None,
                tries: 0,
            });
            return;
        }
        self.follow = Follow::Ending {
            ride,
            riding: self.riding_actor(),
            due_ms: now_ms.saturating_add(CART_END_GRACE_MS),
            give_up_ms: now_ms.saturating_add(CART_OWN_RIDE_LONGEST_MS),
        };
    }

    pub fn step(&mut self, facts: &OwnCartFacts) -> CartFollowStep {
        match self.follow {
            Follow::Idle => CartFollowStep::Nothing,
            Follow::Boarding(boarding) => self.board(facts, boarding),
            Follow::Riding { actor, phase, .. } => self.ride(facts, actor, phase),
            Follow::Ending {
                ride,
                riding,
                due_ms,
                give_up_ms,
            } => self.end(facts, ride, riding, due_ms, give_up_ms),
        }
    }

    pub fn riding_actor(&self) -> Option<u64> {
        match self.follow {
            Follow::Riding { actor, .. } => Some(actor),
            Follow::Ending { riding, .. } => riding,
            _ => None,
        }
    }

    /// This game's cart the host's run steers: only while this player rides the run the host
    /// started (not once the host's ride has ended), and only for the run the message names.
    pub fn steered_cart(&self, run: &CartRun) -> Option<u64> {
        let Follow::Riding {
            actor,
            phase,
            layout_id,
            run_state,
        } = self.follow
        else {
            return None;
        };
        let same_cart = match (layout_id, run.layout_id) {
            (Some(ours), Some(theirs)) => ours == theirs,
            _ => true,
        };
        (run.phase == phase && run.run_state == run_state && same_cart).then_some(actor)
    }

    pub fn steered_actor(&self) -> Option<u64> {
        match self.follow {
            Follow::Riding { actor, .. } => Some(actor),
            _ => None,
        }
    }

    fn board(&mut self, facts: &OwnCartFacts, mut boarding: Boarding) -> CartFollowStep {
        let ride = boarding.ride;
        if facts.phase != Some(ride.phase) {
            self.follow = Follow::Idle;
            return CartFollowStep::OtherMap;
        }
        let Some(cart) = match_ride_cart(facts.carts, &ride) else {
            self.follow = Follow::Idle;
            return CartFollowStep::NoCartHere { ride };
        };
        if facts.rider_on_cart && boarding.asked_ms.is_some() {
            self.follow = Follow::Riding {
                actor: cart.actor,
                phase: ride.phase,
                layout_id: ride.layout_id,
                run_state: ride.to_state,
            };
            return CartFollowStep::Boarded { actor: cart.actor };
        }
        let waited_ms = facts.now_ms.saturating_sub(boarding.arrived_ms);
        if waited_ms >= CART_BOARD_GIVE_UP_MS {
            self.follow = Follow::Idle;
            return CartFollowStep::GaveUpBoarding { waited_ms };
        }
        if let Some(asked_ms) = boarding.asked_ms {
            if facts.now_ms.saturating_sub(asked_ms) < CART_BOARD_CONFIRM_MS {
                return CartFollowStep::Nothing;
            }
            if boarding.tries >= CART_BOARD_TRIES {
                self.follow = Follow::Idle;
                return CartFollowStep::BoardRefused {
                    tries: boarding.tries,
                };
            }
            boarding.placed_ms = None;
            boarding.asked_ms = None;
        }
        if cart.state != CartState::Parked(ride.from_state) {
            self.follow = Follow::Boarding(boarding);
            return CartFollowStep::Park {
                actor: cart.actor,
                state: ride.from_state,
            };
        }
        let Some(placed_ms) = boarding.placed_ms else {
            if !facts.may_place_rider {
                self.follow = Follow::Boarding(boarding);
                return CartFollowStep::Nothing;
            }
            self.follow = Follow::Boarding(Boarding {
                placed_ms: Some(facts.now_ms),
                ..boarding
            });
            let [x, y, z] = ride.boarded_at;
            return CartFollowStep::PlaceRider {
                actor: cart.actor,
                spot: [x, y + CART_RIDER_LIFT_UNITS, z],
            };
        };
        if facts.now_ms.saturating_sub(placed_ms) < CART_BOARD_SETTLE_MS {
            return CartFollowStep::Nothing;
        }
        let try_number = boarding.tries.saturating_add(1);
        self.follow = Follow::Boarding(Boarding {
            asked_ms: Some(facts.now_ms),
            tries: try_number,
            ..boarding
        });
        CartFollowStep::Board {
            actor: cart.actor,
            try_number,
        }
    }

    fn ride(&mut self, facts: &OwnCartFacts, actor: u64, phase: u16) -> CartFollowStep {
        if facts.phase != Some(phase) {
            self.follow = Follow::Idle;
            return CartFollowStep::Nothing;
        }
        let end = if !facts.carts.iter().any(|cart| cart.actor == actor) {
            CartRideEnd::CartGone
        } else if !facts.rider_on_cart {
            CartRideEnd::OnCartFlagCleared
        } else {
            return CartFollowStep::Nothing;
        };
        self.follow = Follow::Idle;
        CartFollowStep::OwnRideEnded { actor, end }
    }

    fn end(
        &mut self,
        facts: &OwnCartFacts,
        ride: CartRide,
        riding: Option<u64>,
        due_ms: u64,
        give_up_ms: u64,
    ) -> CartFollowStep {
        if let Some(actor) = riding
            && !facts.rider_on_cart
        {
            self.follow = Follow::Ending {
                ride,
                riding: None,
                due_ms,
                give_up_ms,
            };
            return CartFollowStep::OwnRideEnded {
                actor,
                end: CartRideEnd::OnCartFlagCleared,
            };
        }
        if facts.now_ms < due_ms {
            return CartFollowStep::Nothing;
        }
        if facts.rider_on_cart {
            // Still on its own ride: parking the cart now would cut it short.
            if facts.now_ms >= give_up_ms {
                self.follow = Follow::Idle;
            }
            return CartFollowStep::Nothing;
        }
        self.follow = Follow::Idle;
        if facts.phase != Some(ride.phase) {
            return CartFollowStep::OtherMap;
        }
        let cart = riding
            .and_then(|actor| facts.carts.iter().find(|cart| cart.actor == actor).copied())
            .or_else(|| match_cart(facts.carts, ride.layout_id, ride.cart_position));
        match cart {
            Some(cart) if cart.state != CartState::Parked(ride.to_state) => {
                CartFollowStep::ParkAtEnd {
                    actor: cart.actor,
                    state: ride.to_state,
                }
            }
            _ => CartFollowStep::Nothing,
        }
    }
}
