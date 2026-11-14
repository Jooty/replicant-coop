use std::sync::{Mutex, PoisonError};

use replicant_game::GameMemory;
use replicant_game::cutscene_skip::{
    CutsceneSkipTracker, EVENT_SKIP_TAKEN_BYTE_RVA, PARTNER_SNAPSHOT_STALE_MS,
};
use replicant_game::marker::EVENT_SCENE_BYTE_RVA;
use replicant_proto::{CutsceneSkipAsk, Message, PartnerActivity};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::memory::InProcessMemory;
use crate::net::{NetShared, NetState, NetThread, net_clock_ms};

pub(super) static TRACKER: Mutex<CutsceneSkipTracker> = Mutex::new(CutsceneSkipTracker::new());

pub fn local_in_cutscene() -> bool {
    TRACKER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .in_scene()
}

pub(crate) fn event_scene_running(build: &VerifiedBuild) -> Option<bool> {
    let memory = InProcessMemory::new(build);
    let base = memory.module_base();
    memory
        .read_u8(base + u64::from(EVENT_SCENE_BYTE_RVA))
        .ok()
        .map(|byte| byte != 0)
}

pub(super) fn observe_scene(build: &VerifiedBuild) -> Option<(bool, bool)> {
    let memory = InProcessMemory::new(build);
    let base = memory.module_base();
    let in_scene = memory
        .read_u8(base + u64::from(EVENT_SCENE_BYTE_RVA))
        .ok()?
        != 0;
    let skip_taken = memory
        .read_u8(base + u64::from(EVENT_SKIP_TAKEN_BYTE_RVA))
        .ok()?
        != 0;
    let mut tracker = TRACKER.lock().unwrap_or_else(PoisonError::into_inner);
    if tracker.observe_scene(in_scene, skip_taken) {
        log_line(&format!(
            "cutscene skip: scene state changed (in scene {in_scene}, skip taken {skip_taken}), asks cleared"
        ));
    }
    Some((in_scene, skip_taken))
}

pub(super) fn is_connected(view: &NetShared) -> bool {
    view.state == NetState::Connected
}

pub(crate) fn partner_in_cutscene(view: &NetShared) -> bool {
    is_connected(view)
        && view.partner_state().is_some_and(|remote| {
            remote.state.activity == PartnerActivity::InCutscene
                && (remote.age().as_millis() as u64) < PARTNER_SNAPSHOT_STALE_MS
        })
}

pub(super) fn partner_name(view: &NetShared) -> String {
    view.partner_state()
        .map(|remote| remote.name.clone())
        .or_else(|| view.partner_name().map(str::to_owned))
        .unwrap_or_else(|| "your partner".to_owned())
}

pub(super) fn send_ask(net: &NetThread) {
    net.send_relayed(Message::CutsceneSkip(CutsceneSkipAsk));
}

pub(super) fn now_ms() -> u64 {
    net_clock_ms()
}
