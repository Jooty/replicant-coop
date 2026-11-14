use std::sync::{LazyLock, Mutex, PoisonError};

use replicant_game::offsets::ACTOR_UNIQUE_ID_OFFSET;
use replicant_game::talk_guard::{PressAsk, PressVerdict, TalkRegistry, judge_press};
use replicant_game::zone_rules::{Companion, Placement};
use replicant_game::{GameMemory, Replicant};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::event_log::post_event;
use crate::memory::InProcessMemory;
use crate::net::{self, NetSharedExt, NetState};
use crate::zone_rules::zone_radius;

static REGISTRY: LazyLock<Mutex<TalkRegistry>> = LazyLock::new(Mutex::default);

pub(super) fn note_name(build: &VerifiedBuild, actor_id: i32, mode: i32, name: &str) {
    let mut registry = REGISTRY.lock().unwrap_or_else(PoisonError::into_inner);
    registry.enter_phase(&phase_of(build));
    registry.register_name(actor_id, mode, name);
}

pub(super) fn note_talk_id(build: &VerifiedBuild, actor_id: i32, mode: i32, talk_id: i32) {
    let mut registry = REGISTRY.lock().unwrap_or_else(PoisonError::into_inner);
    registry.enter_phase(&phase_of(build));
    registry.register_talk_id(actor_id, mode, talk_id);
    log_line(&format!(
        "talk register: actor {actor_id} mode {mode} talk id {talk_id}"
    ));
}

pub(super) fn refuses_important_press(build: &VerifiedBuild, actor: u64) -> bool {
    let memory = InProcessMemory::new(build);
    let Ok(actor_id) = memory.read_i32(actor + ACTOR_UNIQUE_ID_OFFSET) else {
        return false;
    };
    let important = {
        let mut registry = REGISTRY.lock().unwrap_or_else(PoisonError::into_inner);
        registry.enter_phase(&phase_of(build));
        if registry.first_press(actor_id) {
            log_line(&registry.press_line(actor_id));
        }
        registry
            .important_name_of(actor_id)
            .map(|name| format!("{name} is an important talk"))
            .or_else(|| {
                registry
                    .important_talk_id_of(actor_id)
                    .map(|(id, base)| format!("talk {id} ({base}) is an important talk"))
            })
    };
    let Some(description) = important else {
        return false;
    };
    let verdict = partner_verdict();
    let PressVerdict::Wait { distance_units, .. } = &verdict else {
        return false;
    };
    log_line(&format!(
        "talk guard: {description}, waiting for the partner ({distance_units:.0} m away)"
    ));
    post_event(&verdict.prompt_text().unwrap_or_default());
    true
}

fn phase_of(build: &VerifiedBuild) -> String {
    Replicant::new(InProcessMemory::new(build))
        .current_phase()
        .unwrap_or_default()
}

fn partner_verdict() -> PressVerdict {
    let Some(net) = net::handle() else {
        return PressVerdict::Allow;
    };
    let view = net.view();
    let partner = view.partner_state().map(|remote| Companion {
        name: remote.name.clone(),
        placement: Some(Placement {
            phase: remote.state.phase,
            position: remote.state.position,
        }),
        age_ms: remote.age().as_millis() as u64,
    });
    judge_press(&PressAsk {
        connected: view.state == NetState::Connected,
        partner_game_open: !view.partner_watch().is_game_closed(),
        local: view.local.as_ref().map(|local| Placement {
            phase: local.phase,
            position: local.position,
        }),
        partner: partner.as_ref(),
        radius: zone_radius(),
    })
}
