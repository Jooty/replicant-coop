use std::panic;
use std::sync::PoisonError;

use replicant_game::readiness::{classify, read_play_checks};
use replicant_game::stay_together::FollowRole;
use replicant_game::story_calls::{
    PartnerPlace, SceneScope, StoryCallKind, is_event_callback_name, name_hash,
};
use replicant_game::story_relay::{
    CallKey, FireAction, FireFacts, InjectedMatch, NEVER_RELAYED_FUNCTIONS, args_block_len,
    args_hash, pack_hash_of_phase, read_args_block, relay_kind,
};
use replicant_game::story_writes::watches_scene_writes;
use replicant_game::zone_rules::is_zone_exit_callback;
use replicant_game::{GameMemory, PhaseEntry, Replicant};
use replicant_proto::{Message, StoryCall, StoryCallArg};

use crate::diagnostics::log_line;
use crate::memory::InProcessMemory;
use crate::net::NetSharedExt;
use crate::net::{self, NetState, net_clock_ms};
use crate::story_relay::gather;
use crate::story_relay::inject::cancel_queued_call;
use crate::story_relay::pack_log::note_pack_lookup;
use crate::story_relay::settings::shared_story_events;
use crate::story_relay::state::{
    HELD_CALLS, HOOKED_BUILD, LAST_RUN_STARTED_BY_INJECTION_AT_MS, LEDGER,
};
use crate::story_relay::{dummy_hits, message_handlers};
use crate::story_writes::{queued_by_talk_tag, watch_local_scene_writes};

const MAX_HELD_CALLS: usize = 64;

pub fn suppress_fired_call(function_name: &str, arguments: u64) -> bool {
    panic::catch_unwind(|| decide(function_name, arguments)).unwrap_or(false)
}

pub fn ms_since_an_injected_run_started() -> Option<u64> {
    let started = (*LAST_RUN_STARTED_BY_INJECTION_AT_MS
        .lock()
        .unwrap_or_else(PoisonError::into_inner))?;
    Some(net_clock_ms().saturating_sub(started))
}

fn decide(function_name: &str, arguments: u64) -> bool {
    let Some(build) = HOOKED_BUILD.get() else {
        return false;
    };
    let Some(net) = net::handle() else {
        return false;
    };
    let view = net.view();
    let game = Replicant::new(InProcessMemory::new(build));
    let phase_name = game.current_phase().ok();
    let local_phase = phase_name
        .as_deref()
        .and_then(PhaseEntry::index_of)
        .and_then(|index| u16::try_from(index).ok());
    let pack = phase_name.as_deref().map(pack_hash_of_phase);
    if let Some(phase) = local_phase
        && dummy_hits::is_registered(phase, function_name)
    {
        dummy_hits::on_fire(phase, function_name);
        return false;
    }
    if let Some(phase) = local_phase
        && message_handlers::is_host_driven_handler(phase, function_name)
        && handler_path_applies(&view, phase)
    {
        return false;
    }
    let function_hash = name_hash(function_name);
    let kind = pack.and_then(|pack| relay_kind(pack, function_hash));
    note_pack_lookup(pack, kind);
    let relayed_kind = kind.is_some_and(StoryCallKind::is_relayed);
    let call = relayed_kind
        .then(|| read_call(&game, arguments, local_phase, pack, function_name))
        .flatten();
    let key = CallKey {
        function: function_hash,
        args: call.as_ref().map_or(0, |call| args_hash(&call.args)),
    };
    let scope = SceneScope::of(function_name);
    let partner_place = view.partner_place();
    let role = FollowRole::from_slot(view.local_slot);
    let can_gather_partner = role == FollowRole::Host
        && scope == SceneScope::SharedScene
        && relayed_kind
        && local_phase.is_some_and(|phase| gather::admission_is_open(phase, function_name));
    let facts = FireFacts {
        enabled: shared_story_events(),
        connected: view.state == NetState::Connected,
        role,
        local_in_play: classify(read_play_checks(&game)).is_in_play(),
        partner_in_same_phase: view.partner_in_play()
            && view
                .partner_state()
                .is_some_and(|remote| Some(remote.state.phase) == local_phase),
        kind,
        scope,
        partner_here: partner_place.is_same(),
        owned_elsewhere: is_zone_exit_callback(function_name)
            || is_event_callback_name(function_name)
            || NEVER_RELAYED_FUNCTIONS.contains(&function_name)
            || queued_by_talk_tag(function_name.as_bytes()),
        args_relayable: call.is_some(),
        partner_too_far: matches!(partner_place, PartnerPlace::TooFar { .. }),
        can_gather_partner,
    };
    let action = LEDGER
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .on_fire(arguments, &facts, key, net_clock_ms());
    let ids = format!("fn={function_hash:016x} args={:016x}", key.args);
    let action = if action == FireAction::HoldForGatheredPartner {
        let held = call
            .clone()
            .is_some_and(|call| gather::hold_scene_call(build, net, &game, call, partner_place));
        if held {
            action
        } else {
            FireAction::RunWithoutPartner
        }
    } else {
        action
    };
    note_whether_the_run_is_injected(&action);
    if watches_scene_writes(role, facts.connected, scope, action) {
        watch_local_scene_writes(build, function_name);
    }
    match action {
        FireAction::RunLocally => {
            if relayed_kind && facts.enabled && facts.connected && call.is_none() {
                log_line(&format!(
                    "story event: kept local {ids} (arguments not relayable)"
                ));
            }
            false
        }
        FireAction::RunLocalSceneOnly => {
            log_line(&format!(
                "story event: {function_name} is a local scene, runs here only, not relayed {ids}"
            ));
            false
        }
        FireAction::RunWithoutPartner => {
            log_line(&format!(
                "story event: {function_name} runs here only, the partner is {} {ids}",
                partner_place.describe()
            ));
            false
        }
        FireAction::HoldForGatheredPartner => true,
        FireAction::RunInjected(how) => {
            let by = match how {
                InjectedMatch::Pointer => "argument pointer",
                InjectedMatch::Key { .. } => "function and argument hash",
            };
            if let InjectedMatch::Key { queued_arguments } = how {
                cancel_queued_call(queued_arguments);
            }
            if let Some(phase) = local_phase {
                crate::cutscene_skip::note_relayed_scene_played(phase, function_name);
            }
            log_line(&format!(
                "story event: injected call starts {ids} (matched by {by})"
            ));
            false
        }
        FireAction::RunAndAnnounce => {
            if let Some(call) = call {
                net.send_relayed(Message::StoryRun(call));
                log_line(&format!("story event: host ran {ids}, run sent"));
            }
            false
        }
        FireAction::SuppressAndAsk => {
            if let Some(call) = call {
                net.send_relayed(Message::StoryTrigger(call.clone()));
                let mut held = HELD_CALLS.lock().unwrap_or_else(PoisonError::into_inner);
                if held.len() >= MAX_HELD_CALLS {
                    held.remove(0);
                }
                held.retain(|(held_key, _)| *held_key != key);
                held.push((key, call));
                drop(held);
                log_line(&format!("story event: guest held {ids}, trigger sent"));
            }
            true
        }
        FireAction::DropDuplicate => {
            log_line(&format!(
                "story event: dropped a natural fire already run for the partner {ids}"
            ));
            true
        }
    }
}

fn handler_path_applies(view: &net::NetShared, phase: u16) -> bool {
    FollowRole::from_slot(view.local_slot) != FollowRole::Unknown
        && view.partner_in_play()
        && view
            .partner_state()
            .is_some_and(|remote| remote.state.phase == phase)
}

fn note_whether_the_run_is_injected(action: &FireAction) {
    // The script runner runs one popped call at a time, so the latest run to start is the one that locks.
    let started_at = match action {
        FireAction::RunInjected(_) => Some(net_clock_ms()),
        FireAction::RunLocally
        | FireAction::RunLocalSceneOnly
        | FireAction::RunWithoutPartner
        | FireAction::RunAndAnnounce => None,
        FireAction::SuppressAndAsk
        | FireAction::DropDuplicate
        | FireAction::HoldForGatheredPartner => return,
    };
    *LAST_RUN_STARTED_BY_INJECTION_AT_MS
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = started_at;
}

fn read_call(
    game: &Replicant<InProcessMemory>,
    arguments: u64,
    local_phase: Option<u16>,
    pack: Option<u64>,
    function_name: &str,
) -> Option<StoryCall> {
    let args = read_arguments(game.memory(), arguments)?;
    let call = StoryCall {
        phase: local_phase?,
        pack: pack?,
        function: function_name.to_owned(),
        args,
    };
    call.validate().ok()?;
    Some(call)
}

pub(crate) fn read_arguments(
    memory: &InProcessMemory,
    arguments: u64,
) -> Option<Vec<StoryCallArg>> {
    if arguments == 0 {
        return Some(Vec::new());
    }
    let count = memory.read_i32(arguments).ok()?;
    let len = args_block_len(count);
    let Some(len) = len else {
        log_line(&format!(
            "story event: call has {count} arguments, not relayed"
        ));
        return None;
    };
    let mut block = vec![0_u8; len];
    memory.read(arguments, &mut block).ok()?;
    match read_args_block(&block) {
        Ok(args) => Some(args),
        Err(refusal) => {
            log_line(&format!("story event: arguments refused ({refusal:?})"));
            None
        }
    }
}
