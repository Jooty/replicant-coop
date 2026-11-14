use std::sync::{LazyLock, Mutex, PoisonError};

use replicant_game::desync::{DesyncReporter, live_group_rosters};
use replicant_proto::{DesyncDigest, GameReadiness, Message};

use crate::build_gate::VerifiedBuild;
use crate::desync::actors::live_enemy_ids;
use crate::diagnostics::log_line;
use crate::net::{self, NetShared, NetState, net_clock_ms};
use crate::story_progress::story_digest;

static REPORTER: LazyLock<Mutex<DesyncReporter>> = LazyLock::new(Mutex::default);

pub fn tick_desync(build: &VerifiedBuild) {
    let Some(net) = net::handle() else {
        return;
    };
    let arrived = net.take_relayed(|message| matches!(message, Message::Desync(_)));
    let view = net.view();
    let mut reporter = REPORTER.lock().unwrap_or_else(PoisonError::into_inner);
    if view.state != NetState::Connected || view.local.is_none() {
        reporter.reset();
        return;
    }
    if view.effective_local_readiness() != GameReadiness::InPlay {
        return;
    }
    let now = net_clock_ms();
    if reporter.digest_is_due(now)
        && let Some(own) = own_digest(build, &view)
    {
        net.send_relayed(Message::Desync(own));
    }
    let partner = arrived.into_iter().rev().find_map(|message| match message {
        Message::Desync(digest) => Some(digest),
        _ => None,
    });
    let Some(partner) = partner else {
        return;
    };
    let Some(own) = own_digest(build, &view) else {
        return;
    };
    let seen = view.partner_state().map(|remote| remote.state.position);
    for line in reporter.compare(now, &own, &partner, seen) {
        log_line(&line);
    }
}

fn own_digest(build: &VerifiedBuild, view: &NetShared) -> Option<DesyncDigest> {
    let local = view.local.as_ref()?;
    let (groups, groups_not_listed) = live_group_rosters(&live_enemy_ids(build));
    Some(DesyncDigest {
        phase: local.phase,
        position: local.position,
        story: story_digest(),
        groups,
        groups_not_listed,
    })
}
