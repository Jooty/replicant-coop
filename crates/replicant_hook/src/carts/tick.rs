use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::Duration;

use replicant_game::Replicant;
use replicant_game::carts::{
    CART_REFUSED_NOTICE_INTERVAL_MS, CART_RIDING_LOG_INTERVAL_MS, CART_SCAN_INTERVAL_MS,
    CartFollowStep, CartRideEvent, CartRideFacts, CartSteerSample, GUEST_CART_REFUSED_TEXT,
    GuestCartFollow, HostCartRunTrack, HostRideWatch, OwnCartFacts, RiddenCart,
    cart_follow_step_text, cart_frame_set_text, cart_ride_event_text, cart_run_end_sent_text,
    carts_here_text, guest_ride_refused_text, host_cart_run_text, host_run_ended_text,
    read_cart_motion, riding_along_text,
};
use replicant_game::stay_together::{
    FollowRole, JumpPartner, JumpStep, PARTNER_TIMEOUT_MS, distance_between, local_phase_index,
};
use replicant_proto::{CartRun, CartRunProgress, Message, PartnerNotice};

use crate::build_gate::VerifiedBuild;
use crate::carts::board_gate::{board_own_cart, hold_guest_boarding, take_refusal};
use crate::carts::calls::{carts_here, player_on_cart, set_cart_state};
use crate::carts::steer_hook::steer_guest_cart;
use crate::diagnostics::log_line;
use crate::event_log::post_event;
use crate::memory::InProcessMemory;
use crate::net::NetSharedExt;
use crate::net::{self, NetShared, NetThread, net_clock_ms};
use crate::teleport::{bring_guest_along, own_talk_or_script_pause_is_running, place_on_cart};

#[derive(Default)]
struct CartSync {
    last_scan_ms: Option<u64>,
    listed: Option<(u16, Vec<u64>)>,
    host: HostRideWatch,
    guest: GuestCartFollow,
    last_riding_log_ms: u64,
    last_refusal_notice_ms: Option<u64>,
    run_seq: u32,
    host_run: HostCartRunTrack,
    last_steer: Option<CartSteerSample>,
}

static STATE: LazyLock<Mutex<CartSync>> = LazyLock::new(Mutex::default);

pub fn tick_carts(build: &VerifiedBuild) {
    let Some(net) = net::handle() else {
        hold_guest_boarding(false);
        steer_guest_cart(None, None);
        return;
    };
    let view = net.view();
    let role = FollowRole::from_slot(view.local_slot);
    let in_play = view.partner_in_play();
    hold_guest_boarding(role == FollowRole::Guest && in_play);
    let now = net_clock_ms();
    let mut guard = STATE.lock().unwrap_or_else(PoisonError::into_inner);
    let state = &mut *guard;
    report_refusal(net, state, now);
    let cart_messages =
        net.take_relayed(|message| matches!(message, Message::CartRide(_) | Message::CartRun(_)));
    for message in cart_messages {
        if role != FollowRole::Guest {
            continue;
        }
        if let Message::CartRun(run) = message {
            take_host_run(state, &run, now);
            continue;
        }
        if let Message::CartRide(ride) = message {
            log_line(&format!(
                "carts: the host's cart {} went from state {} to {}",
                ride.layout_id
                    .map_or_else(|| "without a layout id".to_owned(), |id| id.to_string()),
                ride.from_state,
                ride.to_state
            ));
            state.guest.host_rode(ride, now);
        }
    }
    if !in_play {
        state.host.reset();
        state.guest.reset();
        state.host_run = HostCartRunTrack::new();
    }
    hand_run_to_cart_update(state, role);
    if !in_play {
        return;
    }
    if state
        .last_scan_ms
        .is_some_and(|last| now.saturating_sub(last) < CART_SCAN_INTERVAL_MS)
    {
        return;
    }
    state.last_scan_ms = Some(now);
    let game = Replicant::new(InProcessMemory::new(build));
    let Some(phase) = local_phase_index(&game) else {
        if role == FollowRole::Host && state.host.riding() {
            let facts = CartRideFacts {
                now_ms: now,
                phase: None,
                carts: &[],
                rider_on_cart: false,
                rider_position: None,
            };
            lead_rides(build, net, &view, state, &facts);
        }
        return;
    };
    let carts = carts_here(build);
    // Logged when the phase or the cart actors change: carts are put by a trap after the map
    // starts, and the old map's cart can outlive the phase change by a scan or two.
    let listed = (
        phase,
        carts.iter().map(|cart| cart.actor).collect::<Vec<_>>(),
    );
    if state.listed.as_ref() != Some(&listed) {
        log_line(&carts_here_text(phase, &carts));
        state.listed = Some(listed);
    }
    let position = game.position().ok().map(|here| [here.x, here.y, here.z]);
    let player = game.playable_actor().unwrap_or(0);
    let rider_on_cart = player_on_cart(build, player);
    match role {
        FollowRole::Host => {
            let facts = CartRideFacts {
                now_ms: now,
                phase: Some(phase),
                carts: &carts,
                rider_on_cart,
                rider_position: position,
            };
            lead_rides(build, net, &view, state, &facts);
        }
        FollowRole::Guest => {
            let facts = OwnCartFacts {
                now_ms: now,
                phase: Some(phase),
                carts: &carts,
                may_place_rider: !own_talk_or_script_pause_is_running(build),
                rider_on_cart,
            };
            follow_rides(build, &view, state, &facts, position);
        }
        FollowRole::Unknown => {}
    }
}

fn lead_rides(
    build: &VerifiedBuild,
    net: &NetThread,
    view: &NetShared,
    state: &mut CartSync,
    facts: &CartRideFacts,
) {
    let ridden_before = state.host.ridden_cart();
    for event in state.host.observe(facts) {
        let told = match event {
            CartRideEvent::Started(ride)
            | CartRideEvent::Ended {
                parked: Some(ride), ..
            } => {
                net.send_relayed(Message::CartRide(ride));
                true
            }
            CartRideEvent::Ended { parked: None, .. } => false,
        };
        log_line(&cart_ride_event_text(&event, told));
        if let CartRideEvent::Ended { end, .. } = event
            && let Some(ridden) = ridden_before
        {
            send_run(net, state, &ridden, CartRunProgress::Ended(end));
            log_line(&cart_run_end_sent_text(end));
        }
    }
    if let Some(ridden) = state.host.ridden_cart() {
        send_run_progress(build, net, state, &ridden, facts.now_ms);
    }
    let partner = view
        .partner_state()
        .filter(|remote| remote.age() <= Duration::from_millis(PARTNER_TIMEOUT_MS))
        .map(|remote| JumpPartner {
            position: remote.state.position,
            phase: remote.state.phase,
        });
    if let JumpStep::BringPartner {
        from,
        to,
        partner_away,
    } = state
        .host
        .pull_due(facts.now_ms, facts.rider_position, partner)
    {
        log_line(&format!(
            "carts: the ride ended {partner_away:.0} units from the guest, bringing the guest to its end"
        ));
        bring_guest_along(build, from, to, partner_away);
    }
}

fn send_run_progress(
    build: &VerifiedBuild,
    net: &NetThread,
    state: &mut CartSync,
    ridden: &RiddenCart,
    now_ms: u64,
) {
    let Some(motion) = read_cart_motion(&InProcessMemory::new(build), ridden.actor) else {
        return;
    };
    send_run(
        net,
        state,
        ridden,
        CartRunProgress::Moving {
            frame: motion.frame,
            rate: motion.rate,
        },
    );
    if now_ms.saturating_sub(state.last_riding_log_ms) >= CART_RIDING_LOG_INTERVAL_MS {
        state.last_riding_log_ms = now_ms;
        log_line(&host_cart_run_text(ridden, &motion));
    }
}

fn send_run(net: &NetThread, state: &mut CartSync, ridden: &RiddenCart, progress: CartRunProgress) {
    state.run_seq = state.run_seq.wrapping_add(1);
    net.send_relayed(Message::CartRun(ridden.run(state.run_seq, progress)));
}

fn take_host_run(state: &mut CartSync, run: &CartRun, now_ms: u64) {
    match run.progress {
        CartRunProgress::Moving { frame, rate } => {
            if state.guest.steered_cart(run).is_some() {
                state.host_run.receive(run.seq, frame, rate, now_ms);
            }
        }
        CartRunProgress::Ended(end) => {
            let was_steered = state.host_run.latest().is_some();
            state.host_run.end(run.seq);
            log_line(&host_run_ended_text(end, was_steered));
        }
    }
}

fn hand_run_to_cart_update(state: &mut CartSync, role: FollowRole) {
    let actor = if role == FollowRole::Guest {
        state.guest.steered_actor()
    } else {
        None
    };
    if actor.is_none() {
        state.host_run.clear();
    }
    let (last, frame_set) = steer_guest_cart(actor, state.host_run.latest());
    state.last_steer = last;
    if let Some(set) = frame_set {
        log_line(&cart_frame_set_text(set.from, set.to, set.gap));
    }
}

fn follow_rides(
    build: &VerifiedBuild,
    view: &NetShared,
    state: &mut CartSync,
    facts: &OwnCartFacts,
    position: Option<[f32; 3]>,
) {
    let step = state.guest.step(facts);
    if let Some(text) = cart_follow_step_text(&step) {
        log_line(&text);
    }
    match step {
        CartFollowStep::Park { actor, state: to }
        | CartFollowStep::ParkAtEnd { actor, state: to } => {
            if !set_cart_state(build, actor, to) {
                log_line(&format!("carts: cart {actor:#x} is gone, nothing done"));
            }
        }
        CartFollowStep::PlaceRider { spot, .. } => place_on_cart(build, spot),
        CartFollowStep::Board { actor, .. } => {
            if !board_own_cart(build, actor) {
                log_line(&format!(
                    "carts: cart {actor:#x} is gone or the cart hook is off, not boarded"
                ));
            }
        }
        CartFollowStep::Nothing
        | CartFollowStep::Boarded { .. }
        | CartFollowStep::BoardRefused { .. }
        | CartFollowStep::OwnRideEnded { .. }
        | CartFollowStep::NoCartHere { .. }
        | CartFollowStep::GaveUpBoarding { .. }
        | CartFollowStep::OtherMap => {}
    }
    log_riding(view, state, facts, position);
}

fn log_riding(
    view: &NetShared,
    state: &mut CartSync,
    facts: &OwnCartFacts,
    position: Option<[f32; 3]>,
) {
    let Some(actor) = state.guest.riding_actor() else {
        return;
    };
    if facts.now_ms.saturating_sub(state.last_riding_log_ms) < CART_RIDING_LOG_INTERVAL_MS {
        return;
    }
    state.last_riding_log_ms = facts.now_ms;
    let cart = facts
        .carts
        .iter()
        .find(|cart| cart.actor == actor)
        .map(|cart| cart.state);
    let away_from_host = view
        .partner_state()
        .and_then(|remote| position.zip(Some(remote.state.position)))
        .and_then(|(here, there)| distance_between(here, there));
    log_line(&riding_along_text(
        facts.rider_on_cart,
        cart,
        away_from_host,
        state.host_run.latest().as_ref(),
        state.last_steer.as_ref(),
    ));
}

fn report_refusal(net: &NetThread, state: &mut CartSync, now: u64) {
    if take_refusal().is_none() {
        return;
    }
    let due = state
        .last_refusal_notice_ms
        .is_none_or(|last| now.saturating_sub(last) >= CART_REFUSED_NOTICE_INTERVAL_MS);
    if !due {
        return;
    }
    state.last_refusal_notice_ms = Some(now);
    log_line(&guest_ride_refused_text());
    post_event(GUEST_CART_REFUSED_TEXT);
    net.send_relayed(Message::Notice(PartnerNotice::WantsToRideTheCart));
}
