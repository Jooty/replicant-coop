use std::sync::{Mutex, PoisonError};

use replicant_game::Replicant;
use replicant_game::lift_rides::{
    HOST_PARTNER_CALLED_LIFT_TEXT, HostBusy, HostLiftRide, HostRideFacts, HostRideStep,
};
use replicant_game::marker::read_world_flags;
use replicant_game::stay_together::local_phase_index;
use replicant_proto::{LiftCall, LiftCallAnswer, LiftCallOutcome, Message};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::event_log::post_event;
use crate::game_hooks::{run_warp_pick, script_pause_in_force};
use crate::memory::InProcessMemory;
use crate::net::{self, net_clock_ms};
use crate::stay_together::{close_main_menu, close_pause_menu, main_menu_open, pause_menu_open};
use crate::teleport::move_beside;

static HOST_RIDE: Mutex<HostLiftRide> = Mutex::new(HostLiftRide::new());
static LAST_WAIT: Mutex<Option<HostBusy>> = Mutex::new(None);

pub(super) fn on_call(build: &VerifiedBuild, call: LiftCall) {
    let game = Replicant::new(InProcessMemory::new(build));
    let phase_name = game.current_phase().ok();
    let host_phase = local_phase_index(&game).zip(phase_name.as_deref());
    let received = HOST_RIDE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .receive(call, host_phase, net_clock_ms());
    match received {
        Ok(()) => {
            post_event(HOST_PARTNER_CALLED_LIFT_TEXT);
            log_line(&format!(
                "lift ride: the partner called the lift (point {}, from {}) at {:.1}, {:.1}, {:.1}",
                call.point, call.origin, call.position[0], call.position[1], call.position[2]
            ));
        }
        Err(outcome) => {
            log_line(&format!(
                "lift ride: the partner's call for point {} is refused ({outcome:?})",
                call.point
            ));
            answer(call.phase, call.point, outcome);
        }
    }
}

pub(super) fn tick_host(build: &VerifiedBuild) {
    let mut ride = HOST_RIDE.lock().unwrap_or_else(PoisonError::into_inner);
    if !ride.in_flight() {
        return;
    }
    let call_facts = host_facts(build);
    let step = ride.step(&call_facts);
    drop(ride);
    if !matches!(step, HostRideStep::Wait(_)) {
        *LAST_WAIT.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
    match step {
        HostRideStep::Idle | HostRideStep::Settling | HostRideStep::Riding => {}
        HostRideStep::Wait(busy) => {
            let mut last = LAST_WAIT.lock().unwrap_or_else(PoisonError::into_inner);
            if *last != Some(busy) {
                *last = Some(busy);
                log_line(&format!(
                    "lift ride: waiting to run the partner's pick ({busy:?})"
                ));
            }
        }
        HostRideStep::CloseMenu => {
            log_line("lift ride: closing the host's menu for the partner's lift");
            if main_menu_open(build) {
                close_main_menu(build, "the partner's lift");
            }
            if pause_menu_open(build) {
                close_pause_menu(build, "the partner's lift");
            }
        }
        HostRideStep::WaitForMenu => {}
        HostRideStep::Place(spot) => match move_beside(build, spot) {
            Ok(()) => log_line("lift ride: moved the host to the partner at the lift panel"),
            Err(reason) => log_line(&format!(
                "lift ride: could not move the host to the panel ({reason}), running from here"
            )),
        },
        HostRideStep::Run {
            phase,
            point,
            origin,
        } => run_pick(phase, point, origin),
        HostRideStep::Refuse {
            phase,
            point,
            outcome,
        } => {
            log_line(&format!(
                "lift ride: the partner's lift to point {point} was not started ({outcome:?})"
            ));
            answer(phase, point, outcome);
        }
        HostRideStep::Arrived { end, waited_ms } => log_line(&format!(
            "lift ride: the host arrived ({end:?}) {waited_ms} ms after the pick ran"
        )),
        HostRideStep::LostTrack { waited_ms } => log_line(&format!(
            "lift ride: the ride had not ended {waited_ms} ms after the pick ran, stopped watching"
        )),
    }
}

fn run_pick(phase: u16, point: u16, origin: u16) {
    crate::fade_sync::arm_host_zone_change();
    if run_warp_pick(i32::from(point), i32::from(origin)) {
        log_line(&format!(
            "lift ride: running the partner's pick on the host: SelectWarpPoint({point}, {origin})"
        ));
        answer(phase, point, LiftCallOutcome::Riding);
    } else {
        log_line("lift ride: the warp pick hook is not installed, the lift was not started");
        answer(phase, point, LiftCallOutcome::HostBusy);
    }
}

fn host_facts(build: &VerifiedBuild) -> HostRideFacts {
    let game = Replicant::new(InProcessMemory::new(build));
    let flags = read_world_flags(game.memory()).ok();
    let script_pause = script_pause_in_force();
    let busy = match flags {
        Some(flags) if flags.event_scene => Some(HostBusy::Scene),
        Some(flags) if flags.talk_window_open => Some(HostBusy::Talk),
        _ if script_pause => Some(HostBusy::ScriptPause),
        _ => None,
    };
    HostRideFacts {
        now_ms: net_clock_ms(),
        phase: local_phase_index(&game),
        busy,
        menu_open: main_menu_open(build) || pause_menu_open(build),
        script_pause,
    }
}

fn answer(phase: u16, point: u16, outcome: LiftCallOutcome) {
    if let Some(net) = net::handle() {
        net.send_relayed(Message::LiftCallAnswer(LiftCallAnswer {
            phase,
            point,
            outcome,
        }));
    }
}
