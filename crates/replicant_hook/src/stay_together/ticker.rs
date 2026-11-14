use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use replicant_game::fight_sync::{SlotState, script_slot_state};
use replicant_game::fix_buttons::{FixButton, host_target_for_session, rejoin_target};
use replicant_game::readiness::{classify, read_play_checks};
use replicant_game::stay_together::{
    AnnounceConditions, FollowConditions, FollowRole, FollowState, FollowStep, HostAnnouncer,
    HostViewFacts, HostViewReading, HostViewTracker, JumpFacts, JumpPartner, JumpStep,
    PARTNER_TIMEOUT_MS, PhaseTarget, SceneEndFacts, SceneEndOwed, SceneEndWatch, ScriptJumpWatch,
    WaitReason, announced_target, check_partner_reachable, choose_spot, local_phase_index,
};
use replicant_game::{PHASE_TABLE, Position, Replicant};
use replicant_proto::{FollowPhase, Message, NO_SIDE_VIEW_DOOR, PartnerNotice};

use crate::actions::{self, Action};
use crate::build_gate::VerifiedBuild;
use crate::cutscene_skip::{control_slots_free_now, event_scene_running, partner_end_waiting_for};
use crate::diagnostics::log_line;
use crate::event_log::post_event;
use crate::fight_sync::script_manager_state;
use crate::game_hooks::{self, own_side_view_door};
use crate::memory::InProcessMemory;
use crate::net::NetSharedExt;
use crate::net::{self, NetShared, NetState};
use crate::stay_together::arrival::{ArrivalState, keep_safe};
use crate::stay_together::free_to_act::game_says_free_to_act;
use crate::stay_together::menu_close::{close_main_menu, main_menu_open};
use crate::stay_together::settings::{
    clear_pending_requests, follow_enabled, take_go_to_partner_request,
    take_rejoin_host_area_request, update_status,
};
use crate::stay_together::side_view::side_view_on;
use crate::stay_together::world_probe::CollisionProbe;

static STARTED_AT: OnceLock<Instant> = OnceLock::new();
static TICKER: Mutex<TickerState> = Mutex::new(TickerState::new());

struct TickerState {
    follow: FollowState,
    announcer: HostAnnouncer,
    arrival: ArrivalState,
    scene_end: SceneEndWatch,
    logged_scene_end_wait: bool,
    last_host_target: Option<PhaseTarget>,
    host_view: HostViewTracker,
    host_facts: Option<(u16, HostViewFacts)>,
    script_jump: ScriptJumpWatch,
}

impl TickerState {
    const fn new() -> Self {
        Self {
            follow: FollowState::new(),
            announcer: HostAnnouncer::new(),
            arrival: ArrivalState::new(),
            scene_end: SceneEndWatch::new(),
            logged_scene_end_wait: false,
            last_host_target: None,
            host_view: HostViewTracker::new(),
            host_facts: None,
            script_jump: ScriptJumpWatch::new(),
        }
    }
}

pub fn tick(build: &VerifiedBuild) {
    let mut state = TICKER.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(net) = net::handle() else {
        state.last_host_target = None;
        clear_pending_requests();
        return;
    };
    let view = net.view();
    let connected_to_peer = view.state == NetState::Connected;
    state.last_host_target = host_target_for_session(state.last_host_target, connected_to_peer);
    if !connected_to_peer {
        clear_pending_requests();
    }
    let now_ms = STARTED_AT.get_or_init(Instant::now).elapsed().as_millis() as u64;
    let game = Replicant::new(InProcessMemory::new(build));
    let local_phase = local_phase_index(&game);
    let role = FollowRole::from_slot(view.local_slot);
    let connected = view.partner_in_play();
    game_hooks::update_menu_no_pause(build, connected);
    crate::paused_hits::tick(net, &view);
    observe_host_view(&mut state, build, &game, role);
    bring_guest_after_script_jump(&mut state, build, &game, &view, role, local_phase, now_ms);
    announce_as_host(&mut state, build, &game, net, role, connected, now_ms);
    if let Some(Message::FollowPhase(follow)) = net
        .take_relayed(|message| matches!(message, Message::FollowPhase(_)))
        .pop()
        && usize::from(follow.phase) < PHASE_TABLE.len()
    {
        let target = PhaseTarget {
            phase: follow.phase,
            point: follow.point,
        };
        state.host_facts = Some((
            follow.phase,
            HostViewFacts {
                in_side_view: follow.in_side_view,
                side_view_door: follow.side_view_door,
                last_normal_spot: follow
                    .last_normal_view
                    .map(|[x, y, z]| Position { x, y, z }),
            },
        ));
        state.last_host_target = Some(target);
        state.follow.host_moved_to(target);
    }
    follow_the_host(&mut state, build, role, connected, local_phase, now_ms);
    let host_facts = state
        .host_facts
        .filter(|(phase, _)| Some(*phase) == local_phase)
        .map(|(_, facts)| facts);
    keep_safe(
        &mut state.arrival,
        build,
        &game,
        &view,
        local_phase,
        host_facts,
        now_ms,
    );
    if take_go_to_partner_request() {
        let outcome = go_to_partner(build, &game, &view, local_phase);
        update_status(|status| status.go_to_partner = outcome);
    }
    if take_rejoin_host_area_request() {
        let outcome = rejoin_host_area(&mut state, build, net, &view, now_ms);
        update_status(|status| status.rejoin = outcome);
    }
}

fn observe_host_view(
    state: &mut TickerState,
    build: &VerifiedBuild,
    game: &Replicant<InProcessMemory>,
    role: FollowRole,
) {
    if role != FollowRole::Host {
        return;
    }
    let reading = HostViewReading {
        phase: local_phase_index(game),
        point: game.arrival_point().unwrap_or(0),
        side_view: side_view_on(build).unwrap_or(false),
        in_play: classify(read_play_checks(game)).is_in_play(),
        free_to_act: game_says_free_to_act(build).unwrap_or(false),
        position: game.position().ok(),
    };
    if let Some(point) = state.host_view.observe(&reading) {
        log_line(&format!(
            "stay together: point {point} put you in a side view on arrival, announcing point 0 instead"
        ));
    }
}

fn bring_guest_after_script_jump(
    state: &mut TickerState,
    build: &VerifiedBuild,
    game: &Replicant<InProcessMemory>,
    view: &NetShared,
    role: FollowRole,
    local_phase: Option<u16>,
    now_ms: u64,
) {
    if role != FollowRole::Host || !view.partner_in_play() {
        state.script_jump.reset();
        return;
    }
    let partner = view
        .partner_state()
        .filter(|remote| remote.age() <= Duration::from_millis(PARTNER_TIMEOUT_MS))
        .map(|remote| JumpPartner {
            position: remote.state.position,
            phase: remote.state.phase,
        });
    let facts = JumpFacts {
        now_ms,
        position: game.position().ok().map(|here| [here.x, here.y, here.z]),
        phase: local_phase,
        script_pause_in_force: game_hooks::script_pause_in_force(),
        mod_moved_the_host: crate::teleport::mod_moved_the_player_just_now(),
        partner,
    };
    if let JumpStep::BringPartner {
        from,
        to,
        partner_away,
    } = state.script_jump.observe(&facts)
    {
        if let Some(beat) = crate::kaine::guest_stands_in_beat() {
            log_line(&format!(
                "teleport: a script moved the host, the guest stays at the spot of stand-in beat {beat} (partner {partner_away:.0} away)"
            ));
            return;
        }
        crate::teleport::bring_guest_along(build, from, to, partner_away);
    }
}

fn announce_as_host(
    state: &mut TickerState,
    build: &VerifiedBuild,
    game: &Replicant<InProcessMemory>,
    net: &net::NetThread,
    role: FollowRole,
    connected: bool,
    now_ms: u64,
) {
    let conditions = AnnounceConditions {
        role,
        connected,
        current: announced_target(game).map(|target| PhaseTarget {
            point: state.host_view.announced_point(target),
            ..target
        }),
        now_ms,
    };
    if let Some(target) = state.announcer.poll(&conditions) {
        let facts = state
            .host_view
            .facts(side_view_on(build).unwrap_or(false), own_side_view_door());
        net.send_relayed(Message::FollowPhase(FollowPhase {
            phase: target.phase,
            point: target.point,
            in_side_view: facts.in_side_view,
            side_view_door: facts.side_view_door,
            last_normal_view: facts.last_normal_spot.map(|spot| [spot.x, spot.y, spot.z]),
        }));
    }
}

fn follow_the_host(
    state: &mut TickerState,
    build: &VerifiedBuild,
    role: FollowRole,
    connected: bool,
    local_phase: Option<u16>,
    now_ms: u64,
) {
    let owes_scene_end = observe_scene_end(state, build, role, connected, local_phase, now_ms);
    let conditions = FollowConditions {
        enabled: follow_enabled(),
        role,
        connected,
        free_to_act: game_says_free_to_act(build).unwrap_or(false)
            || crate::fade_sync::guest_holds_fade(),
        owes_scene_end,
        local_phase,
        now_ms,
    };
    let label = match state.follow.step(&conditions) {
        FollowStep::Idle => String::new(),
        FollowStep::Waiting(reason) => {
            if reason == WaitReason::OwesSceneEnd && !state.logged_scene_end_wait {
                state.logged_scene_end_wait = true;
                log_line(&format!(
                    "stay together: waiting for your scene's end before following {}",
                    partner_name()
                ));
            }
            format!("following the host: {}", reason.label())
        }
        FollowStep::Go(target) => {
            announce_follow(target);
            if main_menu_open(build) {
                close_main_menu(build, "before following the host to a new zone");
            }
            state.arrival.arrival.arm(target, now_ms);
            queue_phase_change(target);
            format!("following the host: changing map (point {})", target.point)
        }
    };
    update_status(|status| status.follow = label);
}

fn observe_scene_end(
    state: &mut TickerState,
    build: &VerifiedBuild,
    role: FollowRole,
    connected: bool,
    local_phase: Option<u16>,
    now_ms: u64,
) -> bool {
    if role != FollowRole::Guest || !connected {
        state.scene_end.reset();
        state.logged_scene_end_wait = false;
        return false;
    }
    let facts = SceneEndFacts {
        in_scene: event_scene_running(build).unwrap_or(false),
        mask_free: control_slots_free_now(build),
        slot_idle: script_slot_state(script_manager_state(build)) == SlotState::Idle,
        partner_end_waiting: local_phase.is_some_and(partner_end_waiting_for),
        local_phase,
        now_ms,
    };
    match state.scene_end.observe(&facts) {
        SceneEndOwed::Yes => true,
        SceneEndOwed::No => {
            state.logged_scene_end_wait = false;
            false
        }
        SceneEndOwed::GaveUp { waited_ms } => {
            state.logged_scene_end_wait = false;
            if state.follow.pending().is_some() {
                log_line(&format!(
                    "stay together: your scene's end did not change the map in {waited_ms} ms, following"
                ));
            }
            false
        }
    }
}

fn partner_name() -> String {
    net::handle()
        .and_then(|net| net.view().partner_name().map(str::to_owned))
        .unwrap_or_else(|| "the host".to_owned())
}

fn announce_follow(target: PhaseTarget) {
    let Some(entry) = PHASE_TABLE.get(usize::from(target.phase)) else {
        return;
    };
    let host = net::handle()
        .and_then(|net| net.view().partner_name().map(str::to_owned))
        .unwrap_or_else(|| "the host".to_owned());
    post_event(&format!("Following {host} to {}", entry.area));
}

fn queue_phase_change(target: PhaseTarget) {
    let Some(entry) = PHASE_TABLE.get(usize::from(target.phase)) else {
        return;
    };
    match Action::change_phase(entry.name, target.point) {
        Ok(action) => {
            if let Err(error) = actions::enqueue(action) {
                log_line(&format!(
                    "stay together: cannot queue the map change ({error})"
                ));
            }
        }
        Err(error) => log_line(&format!("stay together: bad map change ({error})")),
    }
}

fn rejoin_host_area(
    state: &mut TickerState,
    build: &VerifiedBuild,
    net: &net::NetThread,
    view: &NetShared,
    now_ms: u64,
) -> String {
    let id = FixButton::RejoinHostArea.log_id();
    if FollowRole::from_slot(view.local_slot) != FollowRole::Guest || !view.partner_in_play() {
        log_line(&format!("fix button {id}: refused, not a connected guest"));
        return "not rejoined: you need to be the guest with the host in play".to_owned();
    }
    let partner_phase = view.partner_state().map(|remote| remote.state.phase);
    let free_to_act = game_says_free_to_act(build).unwrap_or(false)
        || crate::fade_sync::guest_holds_fade()
        || main_menu_open(build);
    let target = match rejoin_target(state.last_host_target, partner_phase, free_to_act) {
        Ok(target) => target,
        Err(refusal) => {
            log_line(&format!("fix button {id}: refused, {}", refusal.label()));
            return format!("not rejoined: {}", refusal.label());
        }
    };
    if main_menu_open(build) {
        close_main_menu(build, "before rejoining the host's area");
    }
    state.arrival.arrival.arm(target, now_ms);
    state.follow.change_queued_by_rejoin(target, now_ms);
    queue_phase_change(target);
    net.send_relayed(Message::Notice(PartnerNotice::RejoiningYourArea));
    log_line(&format!(
        "fix button {id}: reloading phase {} point {}",
        target.phase, target.point
    ));
    format!("rejoining the host's area (point {})", target.point)
}

fn go_to_partner(
    build: &VerifiedBuild,
    game: &Replicant<InProcessMemory>,
    view: &NetShared,
    local_phase: Option<u16>,
) -> String {
    let remote = view.partner_state();
    let age_ms = remote.map(|remote| remote.age().as_millis() as u64);
    let partner_phase = remote.map(|remote| remote.state.phase);
    if let Err(refusal) =
        check_partner_reachable(view.partner_in_play(), age_ms, local_phase, partner_phase)
    {
        return format!("not moved: {}", refusal.label());
    }
    let (Some(remote), Ok(local)) = (remote, game.position()) else {
        return "not moved: your position is not readable".to_owned();
    };
    if !game_hooks::teleport_available() {
        return "not moved: the movement hook is not installed".to_owned();
    }
    let [x, y, z] = remote.state.position;
    let probe = CollisionProbe::new(build);
    match choose_spot(Position { x, y, z }, local, &probe) {
        Ok(spot) => {
            game_hooks::request_teleport(spot);
            format!("moving to {:.1}, {:.1}, {:.1}", spot.x, spot.y, spot.z)
        }
        Err(refusal) => format!("not moved: {}", refusal.label()),
    }
}

pub(crate) fn announced_host_in_side_view(phase: u16) -> bool {
    TICKER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .host_facts
        .is_some_and(|(host_phase, facts)| host_phase == phase && facts.in_side_view)
}

pub(crate) fn announced_host_side_view_door(phase: u16) -> u16 {
    TICKER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .host_facts
        .filter(|(host_phase, facts)| *host_phase == phase && facts.in_side_view)
        .map_or(NO_SIDE_VIEW_DOOR, |(_, facts)| facts.side_view_door)
}
