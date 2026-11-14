use std::sync::{Mutex, PoisonError};

use replicant_game::callback_gate::is_end_callback;
use replicant_game::scene_follow::{
    AnnounceDrop, AnnounceVerdict, ComingOver, ComingStep, FollowStep, InjectedCallbacks,
    RecentCallbacks, SceneFollowWatch,
};
use replicant_game::stay_together::FollowRole;
use replicant_game::story_calls::PartnerPlace;
use replicant_game::story_relay::GatherVerdict;
use replicant_game::ui_scenes::{OpenUiScenes, read_open_ui_scenes};
use replicant_game::{PhaseEntry, Replicant};
use replicant_proto::{Message, SceneFollow, StoryCall};

use crate::build_gate::VerifiedBuild;
use crate::cutscene_skip::callback_gate::run_queued_ends_through;
use crate::cutscene_skip::state::partner_in_cutscene;
use crate::diagnostics::log_line;
use crate::game_hooks::slot_holder_name;
use crate::memory::InProcessMemory;
use crate::net::NetSharedExt;
use crate::net::{NetShared, NetState, NetThread, net_clock_ms};
use crate::stay_together::{
    announced_host_in_side_view, announced_host_side_view_door, close_main_menu, close_pause_menu,
    main_menu_open, pause_menu_open,
};
use crate::story_relay::{RelayPath, submit_relay_call};
use crate::teleport::{
    come_to_the_starter, gather_verdict_now, moved_to_the_host_just_now, pull_arrival_is_settling,
};

static RECENT: Mutex<RecentCallbacks> = Mutex::new(RecentCallbacks::new());
static WATCH: Mutex<SceneFollowWatch> = Mutex::new(SceneFollowWatch::new());
static COMING: Mutex<ComingOver> = Mutex::new(ComingOver::new());
static INJECTED: Mutex<InjectedCallbacks> = Mutex::new(InjectedCallbacks::new());
static HOST_WAS_IN_SCENE: Mutex<bool> = Mutex::new(false);

pub fn note_fired_callback(name: &str) {
    RECENT
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .note(name, net_clock_ms());
}

pub(super) fn note_injected(name: &str, now_ms: u64) {
    INJECTED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .add(name, now_ms);
}

pub fn is_injected_duplicate(name: &str) -> bool {
    let duplicate = INJECTED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_duplicate(name, net_clock_ms());
    if duplicate {
        log_line(&format!(
            "scene follow: dropped your own fire of {name}, the host's copy of it already ran"
        ));
    }
    duplicate
}

pub(super) fn tick(build: &VerifiedBuild, net: &NetThread, view: &NetShared, local_in_scene: bool) {
    let received = net.take_relayed(|message| matches!(message, Message::SceneFollow(_)));
    if view.state != NetState::Connected {
        WATCH.lock().unwrap_or_else(PoisonError::into_inner).reset();
        COMING
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .reset();
        *HOST_WAS_IN_SCENE
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = false;
        return;
    }
    match FollowRole::from_slot(view.local_slot) {
        FollowRole::Host => host_tick(build, net, view, local_in_scene),
        FollowRole::Guest => guest_tick(build, view, local_in_scene, received),
        FollowRole::Unknown => {}
    }
}

fn host_tick(build: &VerifiedBuild, net: &NetThread, view: &NetShared, in_scene: bool) {
    let mut was = HOST_WAS_IN_SCENE
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let entered = in_scene && !*was;
    *was = in_scene;
    drop(was);
    if !entered || !view.partner_in_play() {
        return;
    }
    let holder = slot_holder_name();
    let names = RECENT
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .names_before_scene(net_clock_ms());
    let Some(phase) = local_phase(build) else {
        return;
    };
    log_line(&format!(
        "scene follow: you entered a scene (slot holder {}), sent to the guest with names {names:?}",
        holder.as_deref().unwrap_or("none")
    ));
    net.send_relayed(Message::SceneFollow(SceneFollow {
        phase,
        callbacks: names,
    }));
}

fn guest_tick(
    build: &VerifiedBuild,
    view: &NetShared,
    local_in_scene: bool,
    received: Vec<Message>,
) {
    let now = net_clock_ms();
    let partner_place = view.partner_place();
    let mut watch = WATCH.lock().unwrap_or_else(PoisonError::into_inner);
    let local = local_phase(build);
    for message in received {
        let Message::SceneFollow(follow) = message else {
            continue;
        };
        if Some(follow.phase) != local {
            log_line("scene follow: the host's scene is in another phase, ignored");
            continue;
        }
        let host_in_side_view = announced_host_in_side_view(follow.phase);
        let verdict = gather_verdict_now(build, follow.phase, host_in_side_view);
        let announced = COMING
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .on_announce(
                partner_place,
                verdict,
                moved_to_the_host_just_now(),
                follow.callbacks.clone(),
                now,
            );
        match announced {
            AnnounceVerdict::FollowNow => start_following(&mut watch, follow, now),
            AnnounceVerdict::ComeOver { move_needed } => {
                log_line(&format!(
                    "scene follow: the host entered a scene {} units away, coming over first",
                    far_units(partner_place)
                ));
                if move_needed
                    && let Some(remote) = view.partner_state()
                    && let GatherVerdict::Come { across_side_view } = verdict
                {
                    come_to_the_starter(
                        build,
                        remote.state.position,
                        host_in_side_view,
                        announced_host_side_view_door(follow.phase),
                        across_side_view,
                    );
                }
            }
            AnnounceVerdict::AlreadyComing => {}
            AnnounceVerdict::Drop(AnnounceDrop::HostNotFarAway) => log_line(&format!(
                "scene follow: not followed (the host is {})",
                partner_place.describe()
            )),
            AnnounceVerdict::Drop(AnnounceDrop::Stay(reason)) => log_line(&format!(
                "scene follow: not followed ({})",
                reason.describe()
            )),
        }
    }
    let coming = COMING.lock().unwrap_or_else(PoisonError::into_inner).step(
        partner_place,
        !pull_arrival_is_settling(),
        now,
    );
    match coming {
        ComingStep::Idle | ComingStep::Waiting => {}
        ComingStep::Arrived {
            waited_ms,
            callbacks,
        } => {
            log_line(&format!(
                "scene follow: arrived after {waited_ms} ms, following the scene"
            ));
            start_following(
                &mut watch,
                SceneFollow {
                    phase: local.unwrap_or_default(),
                    callbacks,
                },
                now,
            );
        }
        ComingStep::GaveUp { waited_ms } => log_line(&format!(
            "scene follow: not followed (did not arrive within {waited_ms} ms)"
        )),
    }
    watch.set_host_in_scene(partner_in_cutscene(view));
    let dialog_open = main_menu_open(build);
    let step = watch.step(local_in_scene, dialog_open, pause_menu_open(build), now);
    drop(watch);
    match step {
        FollowStep::Idle | FollowStep::Wait => {}
        FollowStep::Followed { waited_ms } => {
            log_line(&format!(
                "scene follow: you are in the host's scene ({waited_ms} ms after it began)"
            ));
        }
        FollowStep::CloseDialog { waited_ms } => {
            log_line(&format!(
                "scene follow: still out of the host's scene after {waited_ms} ms with a prompt open, closing it"
            ));
            close_main_menu(build, "to enter the host's scene");
        }
        FollowStep::ClosePauseMenu { waited_ms } => {
            log_line(&format!(
                "scene follow: your pause menu is open {waited_ms} ms after the host's scene began, closing it"
            ));
            close_pause_menu(build, "to enter the host's scene");
        }
        FollowStep::Inject {
            waited_ms,
            callbacks,
        } if !partner_place.is_same() => {
            log_line(&format!(
                "scene follow: not starting {callbacks:?} after {waited_ms} ms, the host is now {}",
                partner_place.describe()
            ));
        }
        FollowStep::Inject {
            waited_ms,
            callbacks,
        } => inject(build, local, waited_ms, &callbacks, now),
        FollowStep::GaveUp { waited_ms } => {
            log_line(&format!(
                "scene follow: gave up after {waited_ms} ms, your game never entered the host's scene"
            ));
        }
    }
}

fn far_units(place: PartnerPlace) -> String {
    match place {
        PartnerPlace::TooFar { units } => format!("{units:.1}"),
        _ => String::from("?"),
    }
}

fn start_following(watch: &mut SceneFollowWatch, follow: SceneFollow, now: u64) {
    let mut names = follow.callbacks;
    if !names.is_empty() && crate::fight_sync::covers_phase(follow.phase) {
        log_line(&format!(
            "scene follow: the fight stream carries this fight's handlers, not starting {names:?}"
        ));
        names.clear();
    }
    log_line(&format!(
        "scene follow: the host entered a scene after {names:?}, your game gets {} ms to enter it",
        replicant_game::scene_follow::SCENE_FOLLOW_GRACE_MS
    ));
    watch.receive(names, now);
}

fn inject(
    build: &VerifiedBuild,
    phase: Option<u16>,
    waited_ms: u64,
    callbacks: &[String],
    now: u64,
) {
    let Some(phase) = phase else {
        return;
    };
    let memory = InProcessMemory::new(build);
    if read_open_ui_scenes(&memory).is_ok_and(OpenUiScenes::pause_menu_open) {
        log_line("scene follow: your pause menu is open, the scene may not start");
    }
    for name in callbacks {
        if is_end_callback(name) && run_queued_ends_through(build, name) {
            continue;
        }
        let call = StoryCall {
            phase,
            pack: 0,
            function: name.clone(),
            args: Vec::new(),
        };
        note_injected(name, now);
        crate::cutscene_skip::callback_gate::allow_next(name);
        let accepted = submit_relay_call(&call, RelayPath::SceneFollow);
        log_line(&format!(
            "scene follow: still out of the host's scene after {waited_ms} ms, starting {name} from the host's step (handed to the relay queue {accepted})"
        ));
    }
}

pub(super) fn local_phase(build: &VerifiedBuild) -> Option<u16> {
    let game = Replicant::new(InProcessMemory::new(build));
    game.current_phase()
        .ok()
        .and_then(|name| PhaseEntry::index_of(&name))
        .and_then(|index| u16::try_from(index).ok())
}
