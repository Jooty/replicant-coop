use std::sync::PoisonError;

use replicant_game::GameMemory;
use replicant_game::Replicant;
use replicant_game::attack_gauge::{
    ATTACK_GAUGE_SWITCHED_OFF_TEXT, BossGaugeState, GaugeAskFacts, GaugeAskRefusal,
    GaugeAskVerdict, GaugeHitFacts, GaugeHitVerdict, GaugeReading, GaugeSwingMiss,
    OpeningReplayReport, gauge_close_text, gauge_hit_applied_text, gauge_hit_dropped_text,
    gauge_open_text, gauge_tally_text, host_ask_gave_up_text, host_ask_refused_text,
    host_ask_replayed_text, node_break_text, read_gauge, read_hp_floor_percent, write_node_hp,
};
use replicant_game::enemies::EnemySighting;
use replicant_proto::{
    AttackGaugeAsk, AttackGaugeHit, AttackGaugeNode, AttackGaugeOpen, EnemyId, Message,
};

use crate::attack_gauge::calls::GaugeCalls;
use crate::attack_gauge::state::{HOST, HostSide, now_ms, off_file_present};
use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::enemies::{OpeningReplayStep, replay_guest_opening_hit};
use crate::memory::InProcessMemory;
use crate::net::NetThread;

/// The host's gauge step, on the game thread after each enemy scan: answers the guest's ask (its
/// game opened a gauge the host lacks) by replaying the guest's opening hit as the host player's
/// own, so the host's game opens its own gauge; applies the guest's gauge hits to the host's own
/// gauge, then sends the gauge's state when it opened, changed or closed.
pub fn tick_host(
    build: &VerifiedBuild,
    net: &NetThread,
    phase: u16,
    seen: &[(EnemyId, EnemySighting)],
) {
    let hits: Vec<AttackGaugeHit> = net
        .take_relayed(|message| matches!(message, Message::AttackGaugeHit(_)))
        .into_iter()
        .filter_map(|message| match message {
            Message::AttackGaugeHit(hit) => Some(hit),
            _ => None,
        })
        .collect();
    let asks: Vec<AttackGaugeAsk> = net
        .take_relayed(|message| matches!(message, Message::AttackGaugeAsk(_)))
        .into_iter()
        .filter_map(|message| match message {
            Message::AttackGaugeAsk(ask) => Some(ask),
            _ => None,
        })
        .collect();
    let game = Replicant::new(InProcessMemory::new(build));
    let mut host = HOST.lock().unwrap_or_else(PoisonError::into_inner);
    let gauge_open = read_gauge(game.memory()).is_some();
    if host.switch.observe(now_ms(), gauge_open, off_file_present) {
        if host.switch.should_log_off(phase) {
            log_line(ATTACK_GAUGE_SWITCHED_OFF_TEXT);
        }
        host.watch.reset();
        host.last_open = None;
        host.pending_ask = None;
        return;
    }
    for ask in &asks {
        answer_ask(build, &mut host, phase, seen, ask);
    }
    step_pending_ask(build, &mut host, seen);
    if !hits.is_empty() {
        apply_guest_hits(build, &game, &mut host, phase, &hits);
    }
    let reading = read_gauge(game.memory());
    let open = reading
        .as_ref()
        .and_then(|reading| shareable_open(&mut host, reading, seen));
    let serial_before = host.watch.serial();
    let now = now_ms();
    let observed = host.watch.observe(now, phase, open);
    if let Some(state) = observed {
        match &state.open {
            Some(open) => {
                if serial_before != Some(state.serial) {
                    log_line(&gauge_open_text(state.serial, open));
                    host.tally.reset();
                }
                host.last_open = Some(open.clone());
            }
            None => {
                log_line(&gauge_close_text(state.serial, host.last_open.as_ref()));
                host.last_open = None;
                host.hits.clear();
                host.last_close = Some((phase, now));
            }
        }
        net.send_relayed(Message::AttackGauge(state));
    }
    if let Some(reading) = &reading
        && host.watch.serial().is_some()
        && host.tally.line_due(now)
    {
        let nodes: Vec<AttackGaugeNode> = reading.nodes.iter().map(|node| node.node).collect();
        log_line(&gauge_tally_text(
            "host",
            host.watch.serial(),
            &nodes,
            &host.tally,
        ));
    }
}

fn answer_ask(
    build: &VerifiedBuild,
    host: &mut HostSide,
    phase: u16,
    seen: &[(EnemyId, EnemySighting)],
    ask: &AttackGaugeAsk,
) {
    let actor_of = |id: EnemyId| {
        seen.iter()
            .find(|(seen_id, _)| *seen_id == id)
            .map(|(_, sighting)| sighting.actor)
    };
    let memory = InProcessMemory::new(build);
    let boss = actor_of(ask.open.boss);
    let partner = ask.open.partner.and_then(actor_of);
    let now = now_ms();
    let facts = GaugeAskFacts {
        local_phase: phase,
        switched_off: host.switch.is_off(),
        host_gauge_open: read_gauge(&memory).is_some(),
        boss_found: boss.is_some(),
        partner_found: partner.is_some(),
        ms_since_host_close: host
            .last_close
            .filter(|(closed_phase, _)| *closed_phase == phase)
            .map(|(_, at)| now.saturating_sub(at)),
    };
    let verdict = host.asks.judge(&facts, ask);
    match verdict {
        GaugeAskVerdict::ReplayOpeningHit => host.pending_ask = Some((ask.clone(), now)),
        GaugeAskVerdict::Refuse(refusal) => log_line(&host_ask_refused_text(ask, refusal)),
    }
}

fn step_pending_ask(build: &VerifiedBuild, host: &mut HostSide, seen: &[(EnemyId, EnemySighting)]) {
    let Some((ask, asked_at)) = host.pending_ask.clone() else {
        return;
    };
    let memory = InProcessMemory::new(build);
    if read_gauge(&memory).is_some() {
        host.pending_ask = None;
        log_line(&host_ask_refused_text(&ask, GaugeAskRefusal::HostGaugeOpen));
        return;
    }
    let Some(boss) = seen
        .iter()
        .find(|(id, _)| *id == ask.open.boss)
        .map(|(_, sighting)| sighting.actor)
    else {
        host.pending_ask = None;
        log_line(&host_ask_refused_text(&ask, GaugeAskRefusal::BossMissing));
        return;
    };
    let ms_since_ask = now_ms().saturating_sub(asked_at);
    match replay_guest_opening_hit(build, boss, ask.open.boss, ms_since_ask) {
        OpeningReplayStep::Wait => {}
        OpeningReplayStep::GiveUp(miss) => {
            host.pending_ask = None;
            log_line(&host_ask_gave_up_text(&ask, miss));
        }
        OpeningReplayStep::CannotReplay(why) => {
            host.pending_ask = None;
            log_line(&format!(
                "attack gauge: guest ask {}: could not replay the guest's opening hit: {why}",
                ask.ask
            ));
        }
        OpeningReplayStep::Replayed(done) => {
            host.pending_ask = None;
            let report = OpeningReplayReport {
                hp_before: done.hp_before,
                hp_after_hit: done.hp_after_hit,
                hp_put_back: done.hp_put_back,
                gauge_opened: read_gauge(&memory).is_some(),
                boss_state: BossGaugeState::read(&memory, boss),
                boss_vtable_rva: memory
                    .read_u64(boss)
                    .ok()
                    .map(|vtable| vtable.wrapping_sub(build.module_base())),
            };
            log_line(&host_ask_replayed_text(&ask, &done.replay, &report));
        }
    }
}

fn shareable_open(
    host: &mut HostSide,
    reading: &GaugeReading,
    seen: &[(EnemyId, EnemySighting)],
) -> Option<AttackGaugeOpen> {
    let id_of = |actor: u64| {
        seen.iter()
            .find(|(_, sighting)| sighting.actor == actor)
            .map(|(id, _)| *id)
    };
    let Some(boss) = id_of(reading.boss) else {
        if host.unshared_boss_logged != Some(reading.boss) {
            host.unshared_boss_logged = Some(reading.boss);
            log_line(&format!(
                "attack gauge: the gauge's boss {:#x} is not a shared enemy, not sent",
                reading.boss
            ));
        }
        return None;
    };
    let partner = match reading.partner {
        None => None,
        Some(actor) => Some(id_of(actor).or_else(|| {
            log_line(&format!(
                "attack gauge: the gauge's second actor {actor:#x} is not a shared enemy, not sent"
            ));
            None
        })?),
    };
    if reading.more_nodes && !host.long_list_logged {
        host.long_list_logged = true;
        log_line("attack gauge: the gauge has more nodes than are sent, only the first are shared");
    }
    Some(AttackGaugeOpen {
        boss,
        partner,
        total_seconds: reading.total_seconds,
        remaining_seconds: reading.remaining_seconds,
        map_points: reading.map_points,
        options: reading.options,
        nodes: reading.nodes.iter().map(|node| node.node).collect(),
    })
}

fn apply_guest_hits(
    build: &VerifiedBuild,
    game: &Replicant<InProcessMemory>,
    host: &mut HostSide,
    phase: u16,
    hits: &[AttackGaugeHit],
) {
    let mut memory = InProcessMemory::new(build);
    let floor_percent = read_hp_floor_percent(&memory).unwrap_or(0);
    for hit in hits {
        let reading = read_gauge(&memory);
        let node = reading
            .as_ref()
            .and_then(|reading| reading.nodes.get(usize::from(hit.node)).copied());
        let facts = GaugeHitFacts {
            local_phase: phase,
            serial: reading.as_ref().and(host.watch.serial()),
            node_hp: node.map(|node| (node.node.hp, node.node.max_hp)),
            floor_percent,
            switched_off: host.switch.is_off(),
        };
        host.tally.sent = host.tally.sent.saturating_add(1);
        let verdict = host.hits.judge(&facts, hit);
        let GaugeHitVerdict::Apply {
            hp_before,
            hp_after,
            breaks,
        } = verdict
        else {
            if let GaugeHitVerdict::Drop(drop) = verdict {
                host.tally.miss(GaugeSwingMiss::DroppedByHost);
                log_line(&gauge_hit_dropped_text(hit, drop));
            }
            continue;
        };
        let (Some(reading), Some(node)) = (reading, node) else {
            continue;
        };
        if !write_node_hp(&mut memory, &node, hp_after) {
            log_line(&format!(
                "attack gauge: cannot write node {} hp at {:#x}",
                hit.node, node.address
            ));
            continue;
        }
        host.tally.applied = host.tally.applied.saturating_add(1);
        log_line(&gauge_hit_applied_text(hit, hp_before, hp_after, breaks));
        if breaks {
            break_node(build, game, &reading, hit.node);
        }
    }
}

fn break_node(
    build: &VerifiedBuild,
    game: &Replicant<InProcessMemory>,
    reading: &GaugeReading,
    node: u8,
) {
    let Ok(player) = game.playable_actor() else {
        log_line(
            "attack gauge: node broke but your player is not readable, the game's break did not run",
        );
        return;
    };
    let before = BossGaugeState::read(game.memory(), reading.boss);
    GaugeCalls::new(build).break_node(reading.dummy, i32::from(node), player);
    let closed = read_gauge(game.memory()).is_none();
    let after = BossGaugeState::read(game.memory(), reading.boss);
    log_line(&node_break_text(node, before, closed, after));
}
