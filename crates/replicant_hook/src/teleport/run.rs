use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use replicant_game::arrival::partner_shares_phase;
use replicant_game::downed::DownedState;
use replicant_game::marker::read_world_flags;
use replicant_game::side_view::DoorMoveAction;
use replicant_game::stay_together::{
    PARTNER_TIMEOUT_MS, choose_spot, distance_between, local_phase_index,
};
use replicant_game::story_relay::{
    GatherFacts, GatherStayReason, GatherVerdict, PullArrival, PullArrivalInputs, PullArrivalStep,
    SCENE_WAITS_FOR_THE_PULLED_PARTNER_MS, gather_verdict,
};
use replicant_game::{Position, Replicant};
use replicant_proto::{GatherAnswer, GatherForScene, GatherReply, Message, TeleportHere};

use crate::build_gate::VerifiedBuild;
use crate::carts::own_player_rides_a_cart;
use crate::diagnostics::log_line;
use crate::downed::downed_status;
use crate::game_hooks::script_pause_in_force;
use crate::memory::InProcessMemory;
use crate::net::NetSharedExt;
use crate::net::{self, net_clock_ms};
use crate::stay_together::{
    CollisionProbe, close_main_menu, close_pause_menu, main_menu_open, pause_menu_open,
    side_view_now,
};
use crate::teleport::door_steps::{
    door_action_for_a_move_here, door_steps_are_running, door_steps_can_run, run_door_action,
    tick_indoor_arrival,
};
use crate::teleport::move_player_with_native;
use crate::teleport::status::{host_can_teleport, remember_outcome};
use crate::teleport::verify;

static LAST_PULL_MOVE_AT_MS: Mutex<Option<u64>> = Mutex::new(None);
static HELD_PULL: Mutex<Option<TeleportHere>> = Mutex::new(None);
static PULL_ARRIVAL: Mutex<PullArrival> = Mutex::new(PullArrival::new());

pub(crate) fn tick(build: &VerifiedBuild) {
    tick_indoor_arrival(build);
    let Some(net) = net::handle() else {
        return;
    };
    let wanted = |message: &Message| {
        matches!(
            message,
            Message::TeleportHere(_) | Message::GatherForScene(_)
        )
    };
    for message in net.take_relayed(wanted) {
        match message {
            Message::TeleportHere(here) => offer_pull(build, here),
            Message::GatherForScene(gather) => gather_for_scene(build, gather),
            _ => {}
        }
    }
    apply_held_pull(build);
    step_pull_arrival(build);
    verify::tick(build);
}

pub(crate) fn bring_guest_along(
    build: &VerifiedBuild,
    from: [f32; 3],
    to: [f32; 3],
    partner_away: f32,
) {
    match send_host_position(build) {
        Ok(()) => log_line(&format!(
            "teleport: a script moved the host ({:.1}, {:.1}, {:.1} -> {:.1}, {:.1}, {:.1}), brought the guest along (partner was {partner_away:.0} away)",
            from[0], from[1], from[2], to[0], to[1], to[2]
        )),
        Err(reason) => log_line(&format!(
            "teleport: a script moved the host, but the guest was not brought along ({reason})"
        )),
    }
}

pub(crate) fn own_talk_or_script_pause_is_running(build: &VerifiedBuild) -> bool {
    let game = Replicant::new(InProcessMemory::new(build));
    let in_own_talk = read_world_flags(game.memory()).is_ok_and(|flags| flags.talk_window_open);
    in_own_talk || script_pause_in_force()
}

fn offer_pull(build: &VerifiedBuild, here: TeleportHere) {
    if !pull_must_wait(build) {
        move_here(build, here);
        return;
    }
    log_line(
        "teleport: the host asked you to come, holding it until your talk, script or cart ride ends",
    );
    *HELD_PULL.lock().unwrap_or_else(PoisonError::into_inner) = Some(here);
}

fn pull_must_wait(build: &VerifiedBuild) -> bool {
    own_talk_or_script_pause_is_running(build) || own_player_rides_a_cart(build)
}

fn apply_held_pull(build: &VerifiedBuild) {
    let mut held = HELD_PULL.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(here) = *held else {
        return;
    };
    let game = Replicant::new(InProcessMemory::new(build));
    if !partner_shares_phase(local_phase_index(&game), here.phase) {
        *held = None;
        log_line("teleport: the held pull is dropped, you are in another map now");
        return;
    }
    if pull_must_wait(build) {
        return;
    }
    *held = None;
    drop(held);
    move_here(build, here);
}

pub(crate) fn bring_guest_here(build: &VerifiedBuild) -> Result<(), String> {
    let result = send_host_position(build);
    report(&result, "teleport: asked the guest to come to the host");
    result
}

pub(crate) fn go_to_guest(build: &VerifiedBuild) -> Result<(), String> {
    let result = move_to_guest(build);
    report(&result, "teleport: asked the game to move to the guest");
    result
}

fn report(result: &Result<(), String>, done: &str) {
    match result {
        Ok(()) => {
            log_line(done);
            remember_outcome(done);
        }
        Err(reason) => {
            log_line(&format!("teleport: refused ({reason})"));
            remember_outcome(reason);
        }
    }
}

fn send_host_position(build: &VerifiedBuild) -> Result<(), String> {
    if !host_can_teleport() {
        return Err(String::from("only a connected host can teleport"));
    }
    let net = net::handle().ok_or("not in a co-op session")?;
    let game = Replicant::new(InProcessMemory::new(build));
    let phase = local_phase_index(&game).ok_or("this game's map is not known")?;
    let position = game.position().map_err(|error| error.to_string())?;
    net.send_relayed(Message::TeleportHere(TeleportHere {
        phase,
        position: [position.x, position.y, position.z],
    }));
    Ok(())
}

fn move_to_guest(build: &VerifiedBuild) -> Result<(), String> {
    if !host_can_teleport() {
        return Err(String::from("only a connected host can teleport"));
    }
    let net = net::handle().ok_or("not in a co-op session")?;
    let view = net.view();
    let remote = view
        .partner_state()
        .filter(|_| view.partner_in_play())
        .filter(|remote| remote.age() <= Duration::from_millis(PARTNER_TIMEOUT_MS))
        .ok_or("the guest's position is not known")?;
    let game = Replicant::new(InProcessMemory::new(build));
    let local_phase = local_phase_index(&game);
    if !partner_shares_phase(local_phase, remote.state.phase) {
        return Err(String::from("the guest is in another map"));
    }
    let [x, y, z] = remote.state.position;
    place(build, &game, Position { x, y, z })
}

fn move_here(build: &VerifiedBuild, here: TeleportHere) {
    let game = Replicant::new(InProcessMemory::new(build));
    let local_phase = local_phase_index(&game);
    if !partner_shares_phase(local_phase, here.phase) {
        log_line("teleport: the host asked you to come, but you are in another map, left alone");
        return;
    }
    let [x, y, z] = here.position;
    match place(build, &game, Position { x, y, z }) {
        Ok(()) => log_line("teleport: asked the game to move to the host"),
        Err(reason) => log_line(&format!("teleport: could not move to the host ({reason})")),
    }
}

pub(crate) fn moved_to_the_host_just_now() -> bool {
    LAST_PULL_MOVE_AT_MS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_some_and(|at| net_clock_ms().saturating_sub(at) < SCENE_WAITS_FOR_THE_PULLED_PARTNER_MS)
}

pub(crate) fn pull_arrival_is_settling() -> bool {
    PULL_ARRIVAL
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_settling()
}

pub(crate) fn gather_verdict_now(
    build: &VerifiedBuild,
    starter_phase: u16,
    starter_in_side_view: bool,
) -> GatherVerdict {
    let game = Replicant::new(InProcessMemory::new(build));
    let flags = read_world_flags(game.memory()).ok();
    gather_verdict(&GatherFacts {
        same_phase: partner_shares_phase(local_phase_index(&game), starter_phase),
        in_side_view: side_view_now(build),
        starter_in_side_view,
        in_own_talk: flags.is_some_and(|flags| flags.talk_window_open),
        in_event_scene: flags.is_some_and(|flags| flags.event_scene),
        downed: downed_status().local == DownedState::Downed,
    })
}

pub(crate) fn come_to_the_starter(
    build: &VerifiedBuild,
    starter_at: [f32; 3],
    starter_in_side_view: bool,
    starter_door: u16,
    across_side_view: bool,
) {
    let game = Replicant::new(InProcessMemory::new(build));
    let Ok(here) = game.position() else {
        log_line("gather: could not move to the partner (your position is not readable)");
        if let Some(phase) = local_phase_index(&game) {
            answer(phase, GatherReply::DidNotArrive);
        }
        return;
    };
    let began = PULL_ARRIVAL
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .begin(
            [here.x, here.y, here.z],
            starter_at,
            starter_in_side_view,
            net_clock_ms(),
        );
    if !began {
        return;
    }
    *LAST_PULL_MOVE_AT_MS
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(net_clock_ms());
    close_menus_for_the_pull(build);
    // Read before the move: the steps belong to the place this player stands in now.
    let door_action = if across_side_view && door_steps_can_run() {
        door_action_for_a_move_here(build, starter_in_side_view, starter_door)
    } else {
        DoorMoveAction::Nothing
    };
    let [x, y, z] = starter_at;
    let target = Position { x, y, z };
    if across_side_view {
        // The exact spot lies inside whatever the game uses there to set the camera view.
        log_line("gather: the partner is in another camera view, moving onto the partner's spot");
        place_exactly(build, &game, target);
        run_door_action(build, door_action, "the mod moved you across a door");
    } else if let Err(reason) = place(build, &game, target) {
        log_line(&format!("gather: could not move to the partner ({reason})"));
    }
}

fn close_menus_for_the_pull(build: &VerifiedBuild) {
    if main_menu_open(build) {
        close_main_menu(build, "the partner starts a scene");
    }
    if pause_menu_open(build) {
        close_pause_menu(build, "the partner starts a scene");
    }
}

fn gather_for_scene(build: &VerifiedBuild, gather: GatherForScene) {
    let across_side_view = match gather_verdict_now(build, gather.phase, gather.in_side_view) {
        GatherVerdict::Stay(reason) => {
            log_line(&format!("gather: staying here ({})", reason.describe()));
            answer(gather.phase, reason.reply());
            return;
        }
        GatherVerdict::Come { across_side_view } => across_side_view,
    };
    let game = Replicant::new(InProcessMemory::new(build));
    let away = game
        .position()
        .ok()
        .and_then(|local| distance_between([local.x, local.y, local.z], gather.position));
    log_line(&format!(
        "gather: the partner starts a scene {} units away, moving to the partner",
        away.map_or_else(|| String::from("?"), |units| format!("{units:.1}"))
    ));
    come_to_the_starter(
        build,
        gather.position,
        gather.in_side_view,
        gather.side_view_door,
        across_side_view,
    );
}

fn step_pull_arrival(build: &VerifiedBuild) {
    let game = Replicant::new(InProcessMemory::new(build));
    let step = PULL_ARRIVAL
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .step(&PullArrivalInputs {
            now_ms: net_clock_ms(),
            position: game.position().ok().map(|here| [here.x, here.y, here.z]),
            in_side_view: side_view_now(build),
            door_steps_running: door_steps_are_running(),
        });
    let answer_now = |reply: GatherReply| {
        if let Some(phase) = local_phase_index(&game) {
            answer(phase, reply);
        }
    };
    match step {
        PullArrivalStep::Idle | PullArrivalStep::Settling => {}
        PullArrivalStep::Ready { waited_ms } => {
            log_line(&format!(
                "gather: beside the partner in the partner's camera view after {waited_ms} ms"
            ));
            answer_now(GatherReply::Ready);
        }
        PullArrivalStep::GoBack { to, waited_ms } => {
            log_line(&format!(
                "gather: {} after {waited_ms} ms, going back to where you were",
                GatherStayReason::SideViewNotGiven.describe()
            ));
            let [x, y, z] = to;
            place_exactly(build, &game, Position { x, y, z });
            answer_now(GatherReply::SideViewNotGiven);
        }
        PullArrivalStep::DidNotArrive { waited_ms } => {
            log_line(&format!(
                "gather: not beside the partner after {waited_ms} ms ({})",
                GatherStayReason::DidNotArrive.describe()
            ));
            answer_now(GatherReply::DidNotArrive);
        }
    }
}

fn answer(phase: u16, reply: GatherReply) {
    if let Some(net) = net::handle() {
        net.send_relayed(Message::GatherAnswer(GatherAnswer { phase, reply }));
    }
}

pub(crate) fn move_beside(build: &VerifiedBuild, spot: [f32; 3]) -> Result<(), String> {
    let game = Replicant::new(InProcessMemory::new(build));
    let [x, y, z] = spot;
    place(build, &game, Position { x, y, z })
}

pub(crate) fn place_on_cart(build: &VerifiedBuild, spot: [f32; 3]) {
    let game = Replicant::new(InProcessMemory::new(build));
    let [x, y, z] = spot;
    place_exactly(build, &game, Position { x, y, z });
}

fn place_exactly(build: &VerifiedBuild, game: &Replicant<InProcessMemory>, spot: Position) {
    let heading_degrees = move_player_with_native(build, game, spot);
    log_line(&format!(
        "teleport: _SetPlayerPosition({:.1}, {:.1}, {:.1}, heading {heading_degrees:.0})",
        spot.x, spot.y, spot.z
    ));
}

fn place(
    build: &VerifiedBuild,
    game: &Replicant<InProcessMemory>,
    target: Position,
) -> Result<(), String> {
    let local = game.position().map_err(|error| error.to_string())?;
    let spot = choose_spot(target, local, &CollisionProbe::new(build)).unwrap_or(target);
    let heading_degrees = move_player_with_native(build, game, spot);
    log_line(&format!(
        "teleport: _SetPlayerPosition({:.1}, {:.1}, {:.1}, heading {heading_degrees:.0})",
        spot.x, spot.y, spot.z
    ));
    Ok(())
}
