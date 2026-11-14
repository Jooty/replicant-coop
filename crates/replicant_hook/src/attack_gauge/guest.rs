use std::sync::PoisonError;
use std::sync::atomic::Ordering;

use replicant_game::attack_gauge::{
    ATTACK_GAUGE_SWITCHED_OFF_TEXT, GaugeReading, GuestGaugeFacts, GuestGaugeStep, HostGaugeView,
    LocalGaugeView, count_nodes, gauge_tally_text, guest_ask_sent_text, guest_close_text,
    guest_gauge_step, guest_node_hp, guest_open_readback_text, guest_open_text, guest_wait_text,
    read_gauge, write_gauge_clock, write_node_hp,
};
use replicant_game::enemies::EnemySighting;
use replicant_proto::{AttackGaugeAsk, AttackGaugeNode, AttackGaugeOpen, EnemyId, Message};

use crate::attack_gauge::calls::GaugeCalls;
use crate::attack_gauge::detours::send_gauge_hit;
use crate::attack_gauge::state::{
    GUEST, GUEST_HOLDS_GAUGE_HITS, GuestSide, now_ms, off_file_present,
};
use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::memory::InProcessMemory;
use crate::net;

/// The guest's gauge step, on the game thread after each enemy scan: the host's gauge decides,
/// the guest opens one like it on its copy of the boss, follows its clock and node hp, and closes
/// its own when the host's closes.
pub fn tick_guest(build: &VerifiedBuild, phase: u16, seen: &[(EnemyId, EnemySighting)]) {
    let Some(net) = net::handle() else {
        return;
    };
    let arrived = net.take_relayed(|message| matches!(message, Message::AttackGauge(_)));
    let mut guest = GUEST.lock().unwrap_or_else(PoisonError::into_inner);
    guest.phase = phase;
    for message in arrived {
        if let Message::AttackGauge(state) = message {
            guest.host = Some(state);
            guest.host_fresh = true;
        }
    }
    let now = now_ms();
    let host_has_open = guest
        .host
        .as_ref()
        .is_some_and(|state| state.open.is_some());
    let switched_off = guest.switch.observe(now, host_has_open, off_file_present);
    if switched_off {
        if guest.switch.should_log_off(phase) {
            log_line(ATTACK_GAUGE_SWITCHED_OFF_TEXT);
        }
        guest.mirrored_serial = None;
        guest.host_fresh = false;
        guest.asked_host = false;
        guest.held_hits.clear();
        GUEST_HOLDS_GAUGE_HITS.store(false, Ordering::Release);
        return;
    }
    let mut memory = InProcessMemory::new(build);
    let reading = read_gauge(&memory);
    if reading.is_none()
        && let Some(serial) = guest.mirrored_serial.take()
    {
        log_line(&format!(
            "attack gauge: number {serial} closed on your side before the host's did"
        ));
    }
    if reading.is_none() {
        forget_own_gauge(&mut guest);
    }
    guest.own_gauge_since_ms = match (&reading, guest.mirrored_serial) {
        (Some(_), None) => Some(guest.own_gauge_since_ms.unwrap_or(now)),
        _ => None,
    };
    let actor_of = |id: EnemyId| {
        seen.iter()
            .find(|(seen_id, _)| *seen_id == id)
            .map(|(_, sighting)| sighting.actor)
    };
    let id_of = |actor: u64| {
        seen.iter()
            .find(|(_, sighting)| sighting.actor == actor)
            .map(|(id, _)| *id)
    };
    let host_open = guest.host.as_ref().and_then(|state| {
        state.open.as_ref().map(|open| HostGaugeView {
            phase: state.phase,
            serial: state.serial,
            boss: open.boss,
            partner: open.partner,
        })
    });
    let boss_copy = host_open.and_then(|host| actor_of(host.boss));
    let partner_copy = host_open.and_then(|host| host.partner).and_then(actor_of);
    let facts = GuestGaugeFacts {
        local_phase: phase,
        host: host_open,
        local: reading.as_ref().map(|reading| LocalGaugeView {
            boss: id_of(reading.boss),
            mirrored_serial: guest.mirrored_serial,
        }),
        boss_copy_found: boss_copy.is_some(),
        partner_copy_found: partner_copy.is_some(),
        local_only_for_ms: guest
            .own_gauge_since_ms
            .map_or(0, |since| now.saturating_sub(since)),
        asked_host: guest.asked_host,
        switched_off,
    };
    let step = guest_gauge_step(&facts);
    if !matches!(step, GuestGaugeStep::Wait(_)) {
        guest.last_wait = None;
    }
    let host_gauge = guest
        .host
        .as_ref()
        .and_then(|state| state.open.clone().map(|open| (state.serial, open)));
    match (step, host_gauge) {
        (GuestGaugeStep::Nothing | GuestGaugeStep::LeaveToGame, _) => {}
        (GuestGaugeStep::Wait(wait), _) => {
            if guest.last_wait != Some(wait) {
                guest.last_wait = Some(wait);
                log_line(&guest_wait_text(host_open.map(|host| host.serial), wait));
            }
        }
        (GuestGaugeStep::AskHost, _) => {
            guest.asked_host = true;
            if let Some(reading) = &reading {
                ask_host(&mut guest, phase, reading, &id_of);
            }
        }
        (GuestGaugeStep::Open, Some((serial, open))) => {
            if guest.gave_up_serial != Some(serial)
                && let Some(boss) = boss_copy
            {
                open_like_host(
                    build,
                    &mut memory,
                    &mut guest,
                    serial,
                    &open,
                    boss,
                    partner_copy,
                );
            }
        }
        (GuestGaugeStep::Mirror, Some((serial, open))) => {
            if guest.mirrored_serial.is_none() {
                guest.mirrored_serial = Some(serial);
                guest.count_mismatch_logged = false;
                log_line(&format!(
                    "attack gauge: took over your own gauge for the host's number {serial}"
                ));
                send_held_hits(&mut guest, serial);
            }
            if guest.host_fresh
                && let Some(reading) = &reading
            {
                follow_host(&mut memory, &mut guest, reading, &open);
            }
        }
        (
            GuestGaugeStep::CloseHostClosed
            | GuestGaugeStep::CloseOtherOpening
            | GuestGaugeStep::CloseHostHasNone,
            _,
        ) => {
            GaugeCalls::new(build).close();
            log_line(&guest_close_text(guest.mirrored_serial, step));
            guest.mirrored_serial = None;
            guest.own_gauge_since_ms = None;
            forget_own_gauge(&mut guest);
        }
        (GuestGaugeStep::Open | GuestGaugeStep::Mirror, None) => {}
    }
    guest.host_fresh = false;
    let after = read_gauge(&memory);
    let holds = (guest.mirrored_serial.is_some() || guest.asked_host) && after.is_some();
    GUEST_HOLDS_GAUGE_HITS.store(holds, Ordering::Release);
    log_tally(&mut guest, now, after.as_ref());
}

fn ask_host(
    guest: &mut GuestSide,
    phase: u16,
    reading: &GaugeReading,
    id_of: &dyn Fn(u64) -> Option<EnemyId>,
) {
    let Some(boss) = id_of(reading.boss) else {
        return;
    };
    let partner = match reading.partner {
        None => None,
        Some(actor) => {
            let Some(id) = id_of(actor) else {
                log_line(&format!(
                    "attack gauge: your game opened a gauge whose second actor {actor:#x} is not a shared enemy, not asked"
                ));
                return;
            };
            Some(id)
        }
    };
    let Some(net) = net::handle() else {
        return;
    };
    guest.next_ask = guest.next_ask.wrapping_add(1);
    let ask = AttackGaugeAsk {
        phase,
        ask: guest.next_ask,
        open: AttackGaugeOpen {
            boss,
            partner,
            total_seconds: reading.total_seconds,
            remaining_seconds: reading.remaining_seconds,
            map_points: reading.map_points,
            options: reading.options,
            nodes: reading.nodes.iter().map(|node| node.node).collect(),
        },
    };
    log_line(&guest_ask_sent_text(&ask));
    net.send_relayed(Message::AttackGaugeAsk(ask));
}

fn send_held_hits(guest: &mut GuestSide, serial: u32) {
    let phase = guest.phase;
    for hit in std::mem::take(&mut guest.held_hits) {
        if send_gauge_hit(phase, serial, hit) {
            guest.tally.sent = guest.tally.sent.saturating_add(1);
        }
    }
}

fn forget_own_gauge(guest: &mut GuestSide) {
    guest.asked_host = false;
    if !guest.held_hits.is_empty() {
        log_line(&format!(
            "attack gauge: {} held hits dropped, the host never opened this gauge",
            guest.held_hits.len()
        ));
        guest.held_hits.clear();
    }
}

fn log_tally(guest: &mut GuestSide, now: u64, reading: Option<&GaugeReading>) {
    let Some(reading) = reading else {
        if guest.tally_shown {
            guest.tally_shown = false;
            guest.tally.reset();
        }
        return;
    };
    if !guest.tally_shown {
        guest.tally_shown = true;
        guest.tally.reset();
    }
    if guest.tally.line_due(now) {
        let nodes: Vec<AttackGaugeNode> = reading.nodes.iter().map(|node| node.node).collect();
        log_line(&gauge_tally_text(
            "guest",
            guest.mirrored_serial,
            &nodes,
            &guest.tally,
        ));
    }
}

fn open_like_host(
    build: &VerifiedBuild,
    memory: &mut InProcessMemory,
    guest: &mut GuestSide,
    serial: u32,
    open: &AttackGaugeOpen,
    boss: u64,
    partner: Option<u64>,
) {
    let calls = GaugeCalls::new(build);
    let give_up = |guest: &mut GuestSide, why: String| {
        guest.gave_up_serial = Some(serial);
        log_line(&format!("attack gauge: cannot open number {serial}: {why}"));
    };
    match count_nodes(memory) {
        Some(0) => {}
        Some(left) => {
            give_up(guest, format!("your gauge list still holds {left} nodes"));
            return;
        }
        None => {
            give_up(guest, "your gauge list is not readable".to_owned());
            return;
        }
    }
    for (expected, node) in (0_i32..).zip(&open.nodes) {
        let index = calls.add_node(node);
        if index != expected {
            calls.close();
            give_up(guest, format!("node {expected} came back as {index}"));
            return;
        }
    }
    if !calls.open(boss, partner, open) {
        calls.close();
        give_up(guest, "the game's open refused".to_owned());
        return;
    }
    let Some(reading) = read_gauge(memory) else {
        calls.close();
        give_up(guest, guest_open_readback_text(serial, None));
        return;
    };
    guest.mirrored_serial = Some(serial);
    guest.count_mismatch_logged = false;
    log_line(&guest_open_text(serial, open));
    follow_host(memory, guest, &reading, open);
    log_line(&guest_open_readback_text(
        serial,
        read_gauge(memory).as_ref(),
    ));
}

fn follow_host(
    memory: &mut InProcessMemory,
    guest: &mut GuestSide,
    reading: &GaugeReading,
    open: &AttackGaugeOpen,
) {
    if !write_gauge_clock(memory, open.total_seconds, open.remaining_seconds) {
        log_line("attack gauge: cannot write your gauge clock");
    }
    if reading.nodes.len() != open.nodes.len() && !guest.count_mismatch_logged {
        guest.count_mismatch_logged = true;
        log_line(&format!(
            "attack gauge: your gauge has {} nodes, the host's {}; only the first are followed",
            reading.nodes.len(),
            open.nodes.len()
        ));
    }
    for (local, host) in reading.nodes.iter().zip(&open.nodes) {
        let hp = guest_node_hp(host.hp);
        if local.node.hp != hp {
            let _ = write_node_hp(memory, local, hp);
        }
    }
}
