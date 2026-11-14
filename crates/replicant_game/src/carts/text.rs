use replicant_proto::CartRide;

use crate::carts::{
    CartFollowStep, CartLeftAlone, CartMotion, CartRideEnd, CartRideEvent, CartSighting, CartState,
    CartSteer, CartSteerSample, HostCartRun, RiddenCart,
};

pub fn carts_here_text(phase: u16, carts: &[CartSighting]) -> String {
    if carts.is_empty() {
        return format!("carts: no cart actor found in phase {phase}");
    }
    let listed: Vec<String> = carts.iter().map(cart_text).collect();
    format!(
        "carts: {} cart(s) in phase {phase}: {}",
        carts.len(),
        listed.join("; ")
    )
}

pub fn cart_ride_event_text(event: &CartRideEvent, partner_told: bool) -> String {
    let told = if partner_told {
        "sent to the guest"
    } else {
        "the guest is not in play, not sent"
    };
    match event {
        CartRideEvent::Started(ride) => format!(
            "carts: host started a ride, cart {} state {} -> {}, cart actor at {}, host boarded at {}, {told}",
            layout_text(ride),
            ride.from_state,
            ride.to_state,
            point_text(ride.cart_position),
            point_text(ride.boarded_at)
        ),
        CartRideEvent::Ended {
            end,
            parked: Some(ride),
            ..
        } => format!(
            "carts: host's ride ended ({}), cart {} state {} -> {}, host at {}, {told}",
            end_text(*end),
            layout_text(ride),
            ride.from_state,
            ride.to_state,
            point_text(ride.boarded_at)
        ),
        CartRideEvent::Ended {
            end,
            parked: None,
            at,
        } => format!(
            "carts: host's ride ended ({}) with the cart not parked at its far end, host at {}, nothing sent; the end pull follows",
            end_text(*end),
            point_text(*at)
        ),
    }
}

pub fn cart_follow_step_text(step: &CartFollowStep) -> Option<String> {
    Some(match step {
        CartFollowStep::Nothing => return None,
        CartFollowStep::Park { actor, state } => {
            format!("carts: your cart {actor:#x} goes to the host's start end (state {state})")
        }
        CartFollowStep::PlaceRider { actor, spot } => format!(
            "carts: standing you where the host boarded, at {}, beside your cart {actor:#x}",
            point_text(*spot)
        ),
        CartFollowStep::Board { actor, try_number } => format!(
            "carts: asking your cart {actor:#x} to take you aboard (the game's own boarding, try {try_number})"
        ),
        CartFollowStep::Boarded { actor } => format!(
            "carts: boarded your cart {actor:#x} by the game's own message (on-cart flag 1)"
        ),
        CartFollowStep::BoardRefused { tries } => format!(
            "carts: your game refused boarding {tries} times (on-cart flag stayed 0); the host brings you at the end"
        ),
        CartFollowStep::OwnRideEnded { actor, end } => {
            format!(
                "carts: your ride on cart {actor:#x} ended ({})",
                end_text(*end)
            )
        }
        CartFollowStep::ParkAtEnd { actor, state } => format!(
            "carts: the host's ride ended, your cart {actor:#x} goes to that end (state {state})"
        ),
        CartFollowStep::NoCartHere { ride } => format!(
            "carts: the host rides cart {} from {}, but no cart of yours matches; the host brings you at the end",
            layout_text(ride),
            point_text(ride.cart_position)
        ),
        CartFollowStep::GaveUpBoarding { waited_ms } => format!(
            "carts: could not stand you on your cart for {waited_ms} ms (talk or script running); the host brings you at the end"
        ),
        CartFollowStep::OtherMap => {
            "carts: the host's ride is in another map, left alone".to_owned()
        }
    })
}

pub fn riding_along_text(
    on_cart: bool,
    cart_state: Option<CartState>,
    away_from_host: Option<f32>,
    host_run: Option<&HostCartRun>,
    steer: Option<&CartSteerSample>,
) -> String {
    let host = host_run.map_or_else(
        || "no host frame".to_owned(),
        |run| format!("host frame {:.1} rate {:.2}", run.frame, run.rate),
    );
    let steered = steer.map_or_else(
        || "not steered yet".to_owned(),
        |sample| {
            format!(
                "your frame {:.1} ({} motion code), {}",
                sample.own_frame,
                motion_code_text(sample.newer_motion),
                steer_text(&sample.steer)
            )
        },
    );
    format!(
        "carts: riding along, on-cart flag {}, your cart {}, {} from the host; {host}, {steered}",
        u8::from(on_cart),
        cart_state.map_or_else(|| "gone".to_owned(), state_text),
        distance_text(away_from_host)
    )
}

pub fn cart_frame_set_text(from: f32, to: f32, gap: f32) -> String {
    format!(
        "carts: your cart was {gap:.1} frames from the host's, its frame set from {from:.1} to {to:.1}"
    )
}

pub fn host_run_ended_text(end: CartRideEnd, was_steered: bool) -> String {
    let steering = if was_steered {
        "your cart is no longer steered and runs out on its own"
    } else {
        "your cart was not being steered"
    };
    format!(
        "carts: the host's ride ended ({}); {steering}",
        end_text(end)
    )
}

pub fn host_cart_run_text(ridden: &RiddenCart, motion: &CartMotion) -> String {
    format!(
        "carts: host riding cart {:#x} (state {}), frame {:.1} rate {:.2} ({} motion code), sent to the guest",
        ridden.actor,
        ridden.run_state,
        motion.frame,
        motion.rate,
        motion_code_text(motion.newer)
    )
}

pub fn cart_run_end_sent_text(end: CartRideEnd) -> String {
    format!(
        "carts: host's ride end ({}) sent to the guest; its cart is no longer steered",
        end_text(end)
    )
}

pub fn guest_ride_refused_text() -> String {
    "carts: you tried to board a cart; the host starts cart rides, refused here and the host was told"
        .to_owned()
}

pub fn partner_wants_to_ride_text(name: &str) -> String {
    format!("{name} wants to ride the cart: board it and you both go")
}

fn cart_text(cart: &CartSighting) -> String {
    let state = state_text(cart.state);
    let layout = cart
        .layout_id
        .map_or_else(|| "no layout id".to_owned(), |id| format!("layout {id}"));
    format!(
        "{:#x} {layout} {state} at {}",
        cart.actor,
        point_text(cart.position)
    )
}

fn state_text(state: CartState) -> String {
    match state {
        CartState::Unset => "unset".to_owned(),
        CartState::Parked(state) => format!("parked {state}"),
        CartState::Running(state) => format!("running {state}"),
    }
}

fn end_text(end: CartRideEnd) -> &'static str {
    match end {
        CartRideEnd::OnCartFlagCleared => "on-cart flag 1 -> 0",
        CartRideEnd::CartParked => "cart parked at its far end",
        CartRideEnd::StateLeftRun => "cart state left its run",
        CartRideEnd::CartGone => "cart actor removed",
        CartRideEnd::PhaseChanged => "the map changed mid-ride",
    }
}

fn steer_text(steer: &CartSteer) -> String {
    match steer {
        CartSteer::LeaveAlone(why) => format!("left alone ({})", left_alone_text(*why)),
        CartSteer::Rate { rate, gap } => format!("gap {gap:.1} frames, rate set {rate:.2}"),
        CartSteer::SetFrame { frame, rate, gap } => {
            format!("gap {gap:.1} frames, frame set {frame:.1}, rate set {rate:.2}")
        }
    }
}

fn left_alone_text(why: CartLeftAlone) -> &'static str {
    match why {
        CartLeftAlone::NoHostRide => "no host run",
        CartLeftAlone::HostSilent => "the host's run went silent",
        CartLeftAlone::HostStill => "the host's cart is still, its game paused",
        CartLeftAlone::AtItsEnd => "past its end frame",
    }
}

fn motion_code_text(newer: bool) -> &'static str {
    if newer { "newer" } else { "old" }
}

fn layout_text(ride: &CartRide) -> String {
    ride.layout_id
        .map_or_else(|| "without a layout id".to_owned(), |id| id.to_string())
}

fn distance_text(units: Option<f32>) -> String {
    units.map_or_else(|| "unknown".to_owned(), |units| format!("{units:.1} units"))
}

fn point_text(point: [f32; 3]) -> String {
    format!("({:.1}, {:.1}, {:.1})", point[0], point[1], point[2])
}
