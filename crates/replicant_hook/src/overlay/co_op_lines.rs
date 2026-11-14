use nc_hook::menu::{CoOpStatus, MenuNote, PlayerLine};
use replicant_game::PHASE_TABLE;

use crate::net::NetSharedExt;
use crate::net::{self, NetShared, NetState, RemotePlayer};

const LINK_OFF_TEXT: &str = "launcher link: off (no nier-coop.toml with a link port)";

pub(super) fn co_op_status() -> CoOpStatus {
    let Some(net) = net::handle() else {
        return CoOpStatus {
            link: LINK_OFF_TEXT.to_owned(),
            connected: false,
            players: Vec::new(),
            ping_ms: None,
        };
    };
    let view = net.view();
    let link = match view.link_port {
        Some(port) => format!("127.0.0.1:{port}, {}", view.state.label()),
        None => view.state.label(),
    };
    let mut players = vec![PlayerLine {
        name: "you".to_owned(),
        detail: format!(
            "phase {}",
            view.local
                .as_ref()
                .map_or("unknown", |local| phase_name(local.phase))
        ),
    }];
    if let Some(name) = view.partner_name() {
        players.push(PlayerLine {
            name: name.to_owned(),
            detail: peer_detail(&view),
        });
    }
    CoOpStatus {
        link,
        connected: view.state == NetState::Connected,
        players,
        ping_ms: view
            .rtt
            .map(|rtt| (rtt.as_secs_f32() * 1000.0).round() as u32),
    }
}

pub(super) fn co_op_notes() -> Vec<MenuNote> {
    let Some(net) = net::handle() else {
        return Vec::new();
    };
    let view = net.view();
    let mut notes = vec![MenuNote::plain("host and join live in the launcher")];
    if view.partner_joining() {
        notes.push(MenuNote::plain(if view.partner_watch().is_game_closed() {
            replicant_game::readiness::PARTNER_GAME_CLOSED_LABEL
        } else {
            replicant_game::readiness::joining_label(view.partner_watch().readiness())
        }));
    }
    notes.push(MenuNote::plain(match view.direct_path {
        Some(true) => "path: direct",
        Some(false) => "path: relay",
        None => "path: unknown",
    }));
    notes
}

fn peer_detail(view: &NetShared) -> String {
    let Some(remote) = view.partner_state() else {
        return "no state received yet".to_owned();
    };
    format!(
        "phase {}, hp {} / {}, {}, last state {} ms ago",
        phase_name(remote.state.phase),
        remote.state.hp,
        remote.state.max_hp,
        distance_text(view, remote),
        remote.age().as_millis()
    )
}

fn phase_name(phase: u16) -> &'static str {
    PHASE_TABLE
        .get(usize::from(phase))
        .map_or("unknown phase", |entry| entry.name)
}

fn distance_text(view: &NetShared, remote: &RemotePlayer) -> String {
    let Some(local) = &view.local else {
        return "distance: unknown".to_owned();
    };
    if local.phase != remote.state.phase {
        return "distance: not in the same phase".to_owned();
    }
    match distance(local.position, remote.state.position) {
        Some(units) => format!("distance: {units:.1}"),
        None => "distance: unknown".to_owned(),
    }
}

fn distance(from: [f32; 3], to: [f32; 3]) -> Option<f32> {
    let squared: f32 = from.iter().zip(to).map(|(a, b)| (a - b) * (a - b)).sum();
    squared.is_finite().then(|| squared.sqrt())
}
