use std::sync::{Mutex, PoisonError};

use replicant_game::containers::{CollectedOpen, remove_only_message};
use replicant_game::door_sync::SnapshotSchedule;
use replicant_game::stay_together::FollowRole;
use replicant_proto::Message;

use crate::build_gate::VerifiedBuild;
use crate::containers::apply::apply_notice;
use crate::containers::state::{HELD, OPENED, WINDOW, now_ms};
use crate::diagnostics::log_line;
use crate::net::NetSharedExt;
use crate::net::{self, NetThread};

static SCHEDULE: Mutex<SnapshotSchedule> = Mutex::new(SnapshotSchedule::new());

pub fn tick(build: &VerifiedBuild) {
    let Some(net) = net::handle() else {
        return;
    };
    let view = net.view();
    let role = FollowRole::from_slot(view.local_slot);
    let local_phase = view.local.as_ref().map(|local| local.phase);
    let entered_new_zone = OPENED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .enter(local_phase);
    if entered_new_zone {
        WINDOW
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .reset();
        log_line(&format!(
            "containers: zone is now {local_phase:?}, opened list cleared"
        ));
    }
    let arrived = net.take_relayed(|message| matches!(message, Message::ContainerOpened(_)));
    let partner_name = view.partner_name().map(str::to_owned).unwrap_or_default();
    for message in arrived {
        if let Message::ContainerOpened(opened) = message {
            apply_notice(build, role, &opened, local_phase, &partner_name);
        }
    }
    let finished = WINDOW
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take_finished(now_ms());
    if let Some(finished) = finished {
        send_collected(net, finished);
    }
    if role == FollowRole::Host {
        let partner_phase = view
            .partner_in_play()
            .then(|| view.partner_state().map(|remote| remote.state.phase))
            .flatten();
        send_opened_ids_when_together(net, local_phase, partner_phase);
    }
}

pub(super) fn send_collected(net: &NetThread, collected: CollectedOpen) {
    let (action_id, phase) = (collected.action_id, collected.phase);
    let Some(message) = collected.into_message() else {
        log_line(&format!(
            "containers: action {action_id} gave no items or gold, nothing sent"
        ));
        return;
    };
    let first_to_open = OPENED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .claim(action_id);
    if !first_to_open {
        log_line(&format!(
            "containers: action {action_id} was already opened this visit, nothing sent"
        ));
        return;
    }
    log_line(&format!(
        "containers: sent action {action_id} in zone {phase}: {} items, {} gold, {} flags",
        message.items.len(),
        message.gold,
        message.flags.len()
    ));
    let is_guest = FollowRole::from_slot(net.view().local_slot) == FollowRole::Guest;
    if is_guest && !message.flags.is_empty() {
        HELD.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .hold(&message.flags, now_ms());
    }
    net.send_relayed(Message::ContainerOpened(message));
}

fn send_opened_ids_when_together(
    net: &NetThread,
    local_phase: Option<u16>,
    partner_phase: Option<u16>,
) {
    let due = {
        let mut schedule = SCHEDULE.lock().unwrap_or_else(PoisonError::into_inner);
        schedule.observe(local_phase, partner_phase, now_ms());
        schedule.take_due(now_ms())
    };
    let Some(phase) = due else {
        return;
    };
    let ids = OPENED.lock().unwrap_or_else(PoisonError::into_inner).ids();
    log_line(&format!(
        "containers: host replays {} opened ids for zone {phase}",
        ids.len()
    ));
    for action_id in ids {
        net.send_relayed(Message::ContainerOpened(remove_only_message(
            phase, action_id,
        )));
    }
}
