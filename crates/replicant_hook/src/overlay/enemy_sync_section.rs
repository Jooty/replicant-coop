use hudhook::imgui::Ui;
use nc_hook::menu_view::parts::{sub_heading, switch_line};
use replicant_proto::EnemyId;

use crate::enemies::{
    EnemySyncRole, EnemySyncStatus, enemy_id_text, enemy_sync_status, set_snap_enabled,
    snap_enabled,
};

const MAX_IDS_LISTED: usize = 8;

pub(super) fn draw_enemy_sync(ui: &Ui) {
    sub_heading(ui, "shared enemies (test readout)");
    let mut snap = snap_enabled();
    if switch_line(ui, "guest: my enemies follow the host's", &mut snap) {
        set_snap_enabled(snap);
    }
    let status = enemy_sync_status();
    match status.role {
        Some(EnemySyncRole::Host) => draw_host(ui, &status),
        Some(EnemySyncRole::Guest) => draw_guest(ui, &status),
        None => {}
    }
    if let Some(reason) = status.idle_reason {
        ui.text(format!("enemy sync: {reason}"));
    }
    draw_combat(ui, &status);
    for event in &status.recent_events {
        ui.text_disabled(event);
    }
}

fn draw_host(ui: &Ui, status: &EnemySyncStatus) {
    ui.text(format!(
        "you are the host: {} enemies here, sent in {} parts",
        status.local_count, status.sent_parts
    ));
}

fn draw_guest(ui: &Ui, status: &EnemySyncStatus) {
    let host = status
        .host_total
        .map_or_else(|| "unknown".to_owned(), |total| total.to_string());
    ui.text(format!(
        "enemies: {} here, {host} at the host ({} received)",
        status.local_count, status.mirrored
    ));
    ui.text(format!("matched: {}", status.matched));
    ui.text(format!("only here: {}", id_list(&status.unmatched_local)));
    ui.text(format!(
        "only at the host: {}",
        id_list(&status.unmatched_remote)
    ));
    match status.worst_error {
        Some((id, error)) => ui.text(format!(
            "worst position error: {error:.2} ({})",
            enemy_id_text(id)
        )),
        None => ui.text("worst position error: none matched"),
    }
    if snap_enabled() {
        ui.text(format!(
            "snapped: {} this frame, {} failed",
            status.snapped, status.snap_failures
        ));
    }
}

fn draw_combat(ui: &Ui, status: &EnemySyncStatus) {
    let counts = status.combat;
    match status.role {
        Some(EnemySyncRole::Host) => ui.text(format!(
            "combat: guest hits {} applied, {} refused; hits on guest {}; kills sent {}; drops sent {}; aggro on guest {}",
            counts.hits_applied,
            counts.hits_refused,
            counts.player_hits_sent,
            counts.kills_sent,
            counts.drops_sent,
            counts.aggro_on_guest
        )),
        Some(EnemySyncRole::Guest) => ui.text(format!(
            "combat: hits sent {}; host hits on you {}; local enemy hits blocked {}; kills {}; drops {} (own held back {}); held {}",
            counts.hits_sent,
            counts.player_hits_applied,
            counts.local_enemy_hits_blocked,
            counts.kills_applied,
            counts.drops_applied,
            counts.copy_drops_blocked,
            counts.held
        )),
        None => {}
    }
}

fn id_list(ids: &[EnemyId]) -> String {
    if ids.is_empty() {
        return "none".to_owned();
    }
    let mut listed: Vec<String> = ids
        .iter()
        .take(MAX_IDS_LISTED)
        .map(|id| enemy_id_text(*id))
        .collect();
    if ids.len() > MAX_IDS_LISTED {
        listed.push(format!("and {} more", ids.len() - MAX_IDS_LISTED));
    }
    listed.join(", ")
}
