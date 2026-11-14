use std::sync::{Mutex, PoisonError};

use replicant_game::Replicant;
use replicant_game::lift_rides::{
    GUEST_LIFT_CALLED_TEXT, GUEST_LIFT_NOT_STARTED_TEXT, GuestCallStep, GuestLiftCall,
    GuestRideEvent, GuestRideFacts, lift_ride,
};
use replicant_game::stay_together::{FollowRole, local_phase_index};
use replicant_proto::{LiftCall, LiftCallAnswer, Message};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::event_log::post_event;
use crate::memory::InProcessMemory;
use crate::net::{self, NetSharedExt, NetState, net_clock_ms};
use crate::teleport::moved_to_the_host_just_now;

static GUEST_CALL: Mutex<GuestLiftCall> = Mutex::new(GuestLiftCall::new());

pub(crate) fn guest_calls_the_lift(build: &VerifiedBuild, point: i32, origin: i32) -> bool {
    let Some(net) = net::handle() else {
        return false;
    };
    let view = net.view();
    if FollowRole::from_slot(view.local_slot) != FollowRole::Guest
        || view.state != NetState::Connected
        || view.partner_watch().is_game_closed()
    {
        return false;
    }
    let game = Replicant::new(InProcessMemory::new(build));
    let Ok(phase_name) = game.current_phase() else {
        return false;
    };
    let Some(ride) = lift_ride(&phase_name, point, origin) else {
        return false;
    };
    let (Some(phase), Ok(here)) = (local_phase_index(&game), game.position()) else {
        log_line("lift ride: picked a lift floor, but this game's map or place is not known");
        return false;
    };
    let step = GUEST_CALL
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .call(net_clock_ms(), phase, ride.point);
    match step {
        GuestCallStep::Send => {
            net.send_relayed(Message::LiftCall(LiftCall {
                phase,
                point: ride.point,
                origin: ride.origin,
                position: [here.x, here.y, here.z],
            }));
            post_event(GUEST_LIFT_CALLED_TEXT);
            log_line(&format!(
                "lift ride: called the lift (point {}, from {}, {}), asked the host to run it",
                ride.point, ride.origin, ride.chain
            ));
        }
        GuestCallStep::DroppedOneInFlight => log_line(&format!(
            "lift ride: picked point {} while a call is in flight, dropped",
            ride.point
        )),
    }
    true
}

pub(super) fn on_answer(answer: LiftCallAnswer) {
    let event = GUEST_CALL
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .answered(net_clock_ms(), answer.point, answer.outcome);
    match event {
        Some(event) => report(event),
        None => log_line(&format!(
            "lift ride: an answer for point {} came with no call waiting, ignored",
            answer.point
        )),
    }
}

pub(super) fn tick_guest(build: &VerifiedBuild) {
    let mut call = GUEST_CALL.lock().unwrap_or_else(PoisonError::into_inner);
    if !call.in_flight() {
        return;
    }
    let game = Replicant::new(InProcessMemory::new(build));
    let event = call.tick(&GuestRideFacts {
        now_ms: net_clock_ms(),
        phase: local_phase_index(&game),
        moved_to_the_host_just_now: moved_to_the_host_just_now(),
    });
    drop(call);
    if let Some(event) = event {
        report(event);
    }
}

fn report(event: GuestRideEvent) {
    match event {
        GuestRideEvent::HostRiding => {
            log_line("lift ride: the host runs the pick and rides, waiting to be taken along");
        }
        GuestRideEvent::Refused(outcome) => {
            post_event(GUEST_LIFT_NOT_STARTED_TEXT);
            log_line(&format!(
                "lift ride: the host could not start it ({outcome:?})"
            ));
        }
        GuestRideEvent::NoAnswer { waited_ms } => {
            post_event(GUEST_LIFT_NOT_STARTED_TEXT);
            log_line(&format!(
                "lift ride: no answer from the host after {waited_ms} ms, call dropped"
            ));
        }
        GuestRideEvent::BroughtAlong { waited_ms } => log_line(&format!(
            "lift ride: arrived, brought to the host {waited_ms} ms after the ride started"
        )),
        GuestRideEvent::ArrivedInNewMap { waited_ms } => log_line(&format!(
            "lift ride: arrived, in the host's new map {waited_ms} ms after the ride started"
        )),
        GuestRideEvent::LostTrack { waited_ms } => log_line(&format!(
            "lift ride: not taken along {waited_ms} ms after the ride started, stopped watching"
        )),
    }
}
