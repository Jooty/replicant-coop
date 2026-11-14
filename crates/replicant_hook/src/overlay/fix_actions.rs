use std::io::{Read, Seek, SeekFrom};

use nc_hook::diagnostics::{log_line, log_path};
use nc_hook::menu::{FixEntry, FixOutcome, MenuNote};
use replicant_game::fix_buttons::{
    ButtonFacts, COPIED_LOG_LINES, FixButton, FixButtonState, LOG_TAIL_READ_BYTES, tail_lines,
};
use replicant_game::stay_together::{FollowRole, distance_between};

use crate::net::{self, NetShared, NetSharedExt};
use crate::puppet::request_ghost_respawn;
use crate::stay_together::{leash, request_go_to_partner, request_rejoin_host_area, stay_status};
use crate::story_progress::request_story_resync;

const FIX_BUTTONS: [(u32, &str, FixButton); 5] = [
    (0, "go to partner", FixButton::GoToPartner),
    (1, "resync story", FixButton::ResyncStory),
    (2, "rejoin the host's area", FixButton::RejoinHostArea),
    (3, "respawn partner", FixButton::RespawnPartner),
    (4, "copy logs", FixButton::CopyLogs),
];

pub(super) fn fix_entries() -> Vec<FixEntry> {
    let facts = button_facts(net::handle().map(|net| net.view()).as_ref());
    FIX_BUTTONS
        .iter()
        .map(|(id, label, button)| FixEntry {
            id: *id,
            label: (*label).to_owned(),
            disabled_reason: match button.state(facts) {
                FixButtonState::Enabled => None,
                FixButtonState::Disabled(reason) => Some(reason.to_owned()),
            },
        })
        .collect()
}

pub(super) fn run_fix(id: u32) -> FixOutcome {
    let Some((_, _, button)) = FIX_BUTTONS.iter().find(|(known, _, _)| *known == id) else {
        return FixOutcome::Refused(format!("unknown fix {id}"));
    };
    log_line(&format!("fix button pressed: {}", button.log_id()));
    match button {
        FixButton::GoToPartner => request_go_to_partner(),
        FixButton::ResyncStory => request_story_resync(),
        FixButton::RejoinHostArea => request_rejoin_host_area(),
        FixButton::RespawnPartner => request_ghost_respawn(),
        FixButton::CopyLogs => return copy_logs(),
    }
    FixOutcome::Done
}

pub(super) fn fix_notes() -> Vec<MenuNote> {
    let Some(net) = net::handle() else {
        return Vec::new();
    };
    let view = net.view();
    let mut notes = Vec::new();
    if far_from_partner(&view) {
        notes.push(MenuNote::warn("you are far from the other player"));
    }
    let status = stay_status();
    for text in [status.go_to_partner, status.rejoin] {
        if !text.is_empty() {
            notes.push(MenuNote::plain(text));
        }
    }
    notes
}

fn button_facts(view: Option<&NetShared>) -> ButtonFacts {
    ButtonFacts {
        role: FollowRole::from_slot(view.and_then(|view| view.local_slot)),
        connected: view.is_some_and(NetShared::partner_in_play),
    }
}

fn far_from_partner(view: &NetShared) -> bool {
    match (&view.local, view.partner_state()) {
        (Some(local), Some(remote)) if local.phase == remote.state.phase => {
            leash().is_exceeded_by(distance_between(local.position, remote.state.position))
        }
        _ => false,
    }
}

fn copy_logs() -> FixOutcome {
    let path = log_path();
    let tail = read_log_tail(path).unwrap_or_default();
    let text = format!("log file: {path}\n{}", tail_lines(&tail, COPIED_LOG_LINES));
    log_line(&format!(
        "fix button copy_logs: copied the last {COPIED_LOG_LINES} lines of {path}"
    ));
    FixOutcome::CopyToClipboard(text)
}

fn read_log_tail(path: &str) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(length.saturating_sub(LOG_TAIL_READ_BYTES)))
        .ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}
