use nc_launcher_core::{CO_OP_CAMPAIGN, StartInputs, start_allowed};

use crate::net::LobbyReady;

pub fn start_enabled(
    connected: bool,
    is_guest: bool,
    already_started: bool,
    own: LobbyReady,
    partner: Option<LobbyReady>,
) -> bool {
    let partner_ready: Vec<bool> = partner.map(|partner| partner.ready).into_iter().collect();
    start_allowed(StartInputs {
        connected,
        viewer_is_guest: is_guest,
        already_started,
        own_ready: own.ready,
        others_ready: &partner_ready,
        rules: CO_OP_CAMPAIGN.rules(),
    })
}

pub fn guest_accepts_start(
    connected: bool,
    is_guest: bool,
    already_started: bool,
    own: LobbyReady,
) -> bool {
    connected && is_guest && !already_started && own.ready
}
