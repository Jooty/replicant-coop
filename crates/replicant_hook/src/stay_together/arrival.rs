use std::time::Duration;

use replicant_game::arrival::{
    ArrivalGuard, ArrivalInputs, ArrivalStep, FallGuard, FallInputs, partner_shares_phase,
};
use replicant_game::downed::{BlockedKillResponse, blocked_kill_response};
use replicant_game::side_view::DoorMoveAction;
use replicant_game::stay_together::{
    HostViewFacts, PARTNER_TIMEOUT_MS, PlacementCheck, PlacementVerdict, ReadBack, WorldProbe,
    choose_spot, decide_placement, distance_between,
};
use replicant_game::{Position, Replicant};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::downed::{request_go_down, take_blocked_kill};
use crate::event_log::post_event;
use crate::game_hooks;
use crate::memory::InProcessMemory;
use crate::net::NetShared;
use crate::net::NetSharedExt;
use crate::stay_together::free_to_act::game_says_free_to_act;
use crate::stay_together::side_view::side_view_on;
use crate::stay_together::world_probe::CollisionProbe;
use crate::teleport::{
    door_action_for_a_move_here, door_steps_can_run, move_player_with_native, run_door_action,
};

pub(super) struct ArrivalState {
    pub(super) arrival: ArrivalGuard,
    fall: FallGuard,
    placing: Option<PendingPlacement>,
}

struct PendingPlacement {
    check: PlacementCheck,
    phase: Option<u16>,
    outside_of: Option<String>,
    door_action: DoorMoveAction,
}

struct PlacementContext<'a> {
    host_facts: Option<HostViewFacts>,
    guest_in_side_view: bool,
    door_action: DoorMoveAction,
    partner_name: &'a str,
}

struct FreshPartner {
    phase: u16,
    position: Position,
}

impl ArrivalState {
    pub(super) const fn new() -> Self {
        Self {
            arrival: ArrivalGuard::new(),
            fall: FallGuard::new(),
            placing: None,
        }
    }
}

pub(super) fn keep_safe(
    state: &mut ArrivalState,
    build: &VerifiedBuild,
    game: &Replicant<InProcessMemory>,
    view: &NetShared,
    local_phase: Option<u16>,
    host_facts: Option<HostViewFacts>,
    now_ms: u64,
) {
    let Ok(local) = game.position() else {
        return;
    };
    let probe = CollisionProbe::new(build);
    let ground_under_local = probe.floor_below(local.x, local.y, local.z).is_some();
    let partner = fresh_partner(view);
    let fall_inputs = FallInputs {
        now_ms,
        local_phase,
        local,
        ground_under_local,
        partner_y_in_same_phase: partner
            .as_ref()
            .filter(|partner| Some(partner.phase) == local_phase)
            .map(|partner| partner.position.y),
    };
    let fallen = state.fall.has_fallen(&fall_inputs);
    let kill_was_blocked = answer_blocked_kill(state, ground_under_local, now_ms);
    let controlled_by_the_game = game_says_free_to_act(build) == Some(false);
    if (fallen || kill_was_blocked) && !controlled_by_the_game {
        recover_from_fall(
            build,
            game,
            state,
            &probe,
            local,
            partner.as_ref(),
            local_phase,
        );
    }
    let arrival_inputs = ArrivalInputs {
        now_ms,
        local_phase,
        partner_phase: partner.as_ref().map(|partner| partner.phase),
        free_to_act: game_says_free_to_act(build).unwrap_or(false),
        distance_to_partner: partner.as_ref().and_then(|partner| {
            distance_between(
                [local.x, local.y, local.z],
                [partner.position.x, partner.position.y, partner.position.z],
            )
        }),
        ground_under_local,
    };
    let same_phase_partner = partner
        .as_ref()
        .filter(|partner| partner_shares_phase(local_phase, partner.phase));
    let guest_in_side_view = side_view_on(build).unwrap_or(false);
    let door_action = match host_facts {
        Some(host) if door_steps_can_run() => {
            door_action_for_a_move_here(build, host.in_side_view, host.side_view_door)
        }
        _ => DoorMoveAction::Nothing,
    };
    let placement = PlacementContext {
        host_facts,
        guest_in_side_view,
        door_action,
        partner_name: view.partner_name().unwrap_or("the host"),
    };
    place_after_follow(
        state,
        build,
        &probe,
        local,
        same_phase_partner,
        &arrival_inputs,
        &placement,
    );
}

fn fresh_partner(view: &NetShared) -> Option<FreshPartner> {
    let remote = view.partner_state()?;
    if !view.partner_in_play() || remote.age() > Duration::from_millis(PARTNER_TIMEOUT_MS) {
        return None;
    }
    let [x, y, z] = remote.state.position;
    Some(FreshPartner {
        phase: remote.state.phase,
        position: Position { x, y, z },
    })
}

fn place_after_follow(
    state: &mut ArrivalState,
    build: &VerifiedBuild,
    probe: &CollisionProbe,
    local: Position,
    partner: Option<&FreshPartner>,
    inputs: &ArrivalInputs,
    context: &PlacementContext,
) {
    if verify_pending_placement(state, build, local, inputs.local_phase, inputs.now_ms) {
        return;
    }
    match state.arrival.step(inputs) {
        ArrivalStep::Idle | ArrivalStep::Waiting => {}
        ArrivalStep::AlreadyBesideHost => log_line("arrival: already beside the host, left alone"),
        ArrivalStep::GaveUp => log_line("arrival: gave up waiting for the host in the same map"),
        ArrivalStep::Place(_) => {
            let Some(partner) = partner else {
                return;
            };
            let verdict = decide_placement(
                context.host_facts,
                context.guest_in_side_view,
                context.door_action,
            );
            match verdict {
                PlacementVerdict::BesideHost => match choose_spot(partner.position, local, probe) {
                    Ok(spot) => begin_placement(state, spot, inputs, None, DoorMoveAction::Nothing),
                    Err(refusal) => log_line(&format!(
                        "arrival: no spot beside the host yet ({})",
                        refusal.label()
                    )),
                },
                PlacementVerdict::BesideHostThroughDoor(door_action) => {
                    match choose_spot(partner.position, local, probe) {
                        Ok(spot) => {
                            log_line(&format!(
                                "arrival: {} is across a door, placing you beside them and running the door's own steps",
                                context.partner_name
                            ));
                            begin_placement(state, spot, inputs, None, door_action);
                        }
                        Err(refusal) => log_line(&format!(
                            "arrival: no spot beside the host yet ({})",
                            refusal.label()
                        )),
                    }
                }
                PlacementVerdict::OutsideSpot(spot) => {
                    post_event(&format!(
                        "{} is inside: use the door to join",
                        context.partner_name
                    ));
                    begin_placement(
                        state,
                        spot,
                        inputs,
                        Some(context.partner_name.to_owned()),
                        DoorMoveAction::Nothing,
                    );
                }
                PlacementVerdict::StayWhereYouAre => {
                    state.arrival.placed();
                    post_event(&format!(
                        "{} is outside: use the door to join",
                        context.partner_name
                    ));
                    log_line(&format!(
                        "arrival: you are in a side view and {} is not, staying where you are",
                        context.partner_name
                    ));
                }
                PlacementVerdict::StayAtEntry => {
                    state.arrival.placed();
                    post_event(&format!(
                        "{} is inside: use the door to join",
                        context.partner_name
                    ));
                    log_line(&format!(
                        "arrival: {} is in a side view and no spot outside it is known, staying at the entry point",
                        context.partner_name
                    ));
                }
            }
        }
    }
}

fn begin_placement(
    state: &mut ArrivalState,
    spot: Position,
    inputs: &ArrivalInputs,
    outside_of: Option<String>,
    door_action: DoorMoveAction,
) {
    if !game_hooks::teleport_available() {
        log_line("arrival: cannot place, the movement hook is not installed");
        return;
    }
    game_hooks::request_teleport(spot);
    state.arrival.placed();
    state.placing = Some(PendingPlacement {
        check: PlacementCheck::requested(spot, inputs.now_ms),
        phase: inputs.local_phase,
        outside_of,
        door_action,
    });
}

fn verify_pending_placement(
    state: &mut ArrivalState,
    build: &VerifiedBuild,
    local: Position,
    local_phase: Option<u16>,
    now_ms: u64,
) -> bool {
    let Some(pending) = state.placing.as_mut() else {
        return false;
    };
    if pending.phase != local_phase {
        state.placing = None;
        return false;
    }
    let spot = pending.check.target();
    let at = format!("{:.1}, {:.1}, {:.1}", spot.x, spot.y, spot.z);
    match pending.check.read_back(local, now_ms) {
        ReadBack::Waiting => {}
        ReadBack::Placed => {
            match &pending.outside_of {
                None => log_line(&format!("arrival: placed beside the host at {at}")),
                Some(name) => log_line(&format!(
                    "arrival: {name} is in a side view, placed at their last spot outside it at {at}"
                )),
            }
            run_door_action(
                build,
                pending.door_action,
                "the mod placed you across a door",
            );
            state.placing = None;
        }
        ReadBack::NotTaken { try_number } => {
            log_line(&format!(
                "arrival: the game did not take the spot (try {try_number})"
            ));
            game_hooks::request_teleport(spot);
        }
        ReadBack::GaveUp { tries } => {
            log_line(&format!(
                "arrival: the game did not take the spot (try {tries})"
            ));
            log_line(&format!("arrival: gave up placing after {tries} tries"));
            state.placing = None;
        }
    }
    true
}

fn answer_blocked_kill(state: &mut ArrivalState, ground_under_local: bool, now_ms: u64) -> bool {
    if !take_blocked_kill() {
        return false;
    }
    match blocked_kill_response(ground_under_local) {
        BlockedKillResponse::GoDown => {
            request_go_down();
            false
        }
        BlockedKillResponse::RecoverFromFall => state.fall.recover_after_blocked_kill(now_ms),
    }
}

fn recover_from_fall(
    build: &VerifiedBuild,
    game: &Replicant<InProcessMemory>,
    state: &ArrivalState,
    probe: &CollisionProbe,
    local: Position,
    partner: Option<&FreshPartner>,
    local_phase: Option<u16>,
) {
    let beside_partner = partner
        .filter(|partner| partner_shares_phase(local_phase, partner.phase))
        .and_then(|partner| choose_spot(partner.position, local, probe).ok());
    match beside_partner.or(state.fall.last_grounded()) {
        Some(spot) => {
            move_player_with_native(build, game, spot);
            let now = game.position().map_or_else(
                |_| "unreadable".to_owned(),
                |at| format!("{:.1}, {:.1}, {:.1}", at.x, at.y, at.z),
            );
            log_line(&format!(
                "fall: moved you to {:.1}, {:.1}, {:.1} (now at {now})",
                spot.x, spot.y, spot.z
            ));
        }
        None => log_line("fall: no ground under you and nowhere known to go back to"),
    }
}
