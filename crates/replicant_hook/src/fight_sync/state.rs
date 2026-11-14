use std::sync::{LazyLock, Mutex, OnceLock, PoisonError};

use replicant_game::fight_sync::{GuestFightStream, HostFightStream, ObjectArmedCalls};
use replicant_game::stay_together::FollowRole;
use replicant_game::{PhaseEntry, Replicant};

use crate::build_gate::VerifiedBuild;
use crate::memory::InProcessMemory;
use crate::net::NetSharedExt;
use crate::net::{self, NetState, net_clock_ms};
use crate::story_relay::shared_story_events;

pub(super) static BUILD: OnceLock<VerifiedBuild> = OnceLock::new();
pub(super) static HOST: LazyLock<Mutex<HostFightStream>> = LazyLock::new(Mutex::default);
pub(super) static GUEST: LazyLock<Mutex<GuestFightStream>> = LazyLock::new(Mutex::default);
pub(super) static REGISTRY_KINDS_BEFORE_POP: Mutex<Vec<i32>> = Mutex::new(Vec::new());
pub(super) static SEEN_RESTARTS: Mutex<Option<(u32, u32)>> = Mutex::new(None);
pub(super) static OBJECT_ARMED: Mutex<ObjectArmedCalls> = Mutex::new(ObjectArmedCalls::new());

#[derive(Clone, Copy)]
pub(super) struct Here {
    pub(super) role: FollowRole,
    pub(super) local_phase: Option<u16>,
    pub(super) partner_in_same_phase: bool,
}

pub(crate) fn install(build: VerifiedBuild) {
    if BUILD.set(build).is_err() {
        return;
    }
    crate::fight_sync::tape::install(build);
    crate::fight_sync::radio_waits::install(build);
    crate::fight_sync::object_arms::install(build);
    crate::fight_sync::prompt_follow::install(build);
    crate::fight_sync::player_waits::install(build);
    crate::fight_sync::scripted_camera::install(build);
    crate::fight_sync::camera_follow::install(build);
    crate::fight_sync::eye_set_callers::install(build);
}

pub(crate) fn covers_phase(phase: u16) -> bool {
    let host_open = HOST
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_open_in(phase);
    host_open
        || GUEST
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_active_in(Some(phase))
}

pub(crate) fn own_call_queued() -> bool {
    let host_queued = HOST
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .has_own_call_queued();
    host_queued
        || GUEST
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .has_own_call_queued()
}

pub(crate) fn partner_call_queued(name: &str) {
    let Some(net) = net::handle() else {
        return;
    };
    if FollowRole::from_slot(net.view().local_slot) != FollowRole::Guest {
        return;
    }
    GUEST
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .partner_call_queued(name, net_clock_ms());
}

pub(super) fn here(build: &VerifiedBuild) -> Option<Here> {
    if !shared_story_events() {
        return None;
    }
    let net = net::handle()?;
    let view = net.view();
    if view.state != NetState::Connected {
        return None;
    }
    let local_phase = current_phase_index(build);
    let partner_in_same_phase = view.partner_in_play()
        && view
            .partner_state()
            .is_some_and(|remote| Some(remote.state.phase) == local_phase);
    Some(Here {
        role: FollowRole::from_slot(view.local_slot),
        local_phase,
        partner_in_same_phase,
    })
}

pub(super) fn current_phase_label(build: &VerifiedBuild) -> Option<String> {
    Replicant::new(InProcessMemory::new(build))
        .current_phase()
        .ok()
}

pub(super) fn current_phase_index(build: &VerifiedBuild) -> Option<u16> {
    current_phase_label(build)
        .as_deref()
        .and_then(PhaseEntry::index_of)
        .and_then(|index| u16::try_from(index).ok())
}
