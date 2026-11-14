use replicant_proto::{AttackGaugeAsk, AttackGaugeHit, AttackGaugeNode, AttackGaugeOpen, EnemyId};

use crate::attack_gauge::{
    BossGaugeState, GaugeAskRefusal, GaugeHitDrop, GaugeReading, GaugeSwingMiss, GaugeTally,
    GuestGaugeStep, GuestGaugeWait, OpeningHitMiss, OpeningHitReplay, OpeningReplayReport,
    boss_gauge_state_text,
};
use crate::enemies::unpack_hit_kind;

pub fn gauge_open_text(serial: u32, open: &AttackGaugeOpen) -> String {
    format!(
        "attack gauge: host opened number {serial} on {}, {} nodes (hp {}), {:.1} s, sent",
        actors_text(open.boss, open.partner),
        open.nodes.len(),
        hp_list(&open.nodes),
        open.total_seconds
    )
}

pub fn gauge_close_text(serial: u32, last: Option<&AttackGaugeOpen>) -> String {
    last.map_or_else(
        || format!("attack gauge: host closed number {serial}, sent"),
        |open| {
            format!(
                "attack gauge: host closed number {serial} with {:.1} s left, nodes hp {}, sent",
                open.remaining_seconds,
                hp_list(&open.nodes)
            )
        },
    )
}

pub fn gauge_hit_applied_text(
    hit: &AttackGaugeHit,
    hp_before: i32,
    hp_after: i32,
    breaks: bool,
) -> String {
    format!(
        "attack gauge: guest hit number {} on node {} of number {} took {}, hp {hp_before} -> {hp_after}{}",
        hit.seq,
        hit.node,
        hit.serial,
        hit.damage,
        if breaks { ", the node broke" } else { "" }
    )
}

pub fn gauge_hit_dropped_text(hit: &AttackGaugeHit, drop: GaugeHitDrop) -> String {
    let reason = match drop {
        GaugeHitDrop::OtherPhase => "another phase",
        GaugeHitDrop::Closed => "the host's gauge is closed",
        GaugeHitDrop::OtherOpening => "another opening",
        GaugeHitDrop::NoSuchNode => "no such node",
        GaugeHitDrop::NodeAlreadyBroken => "the node is already broken",
        GaugeHitDrop::Repeated => "already applied",
    };
    format!(
        "attack gauge: guest hit number {} on node {} of number {} dropped ({reason})",
        hit.seq, hit.node, hit.serial
    )
}

pub fn guest_open_text(serial: u32, open: &AttackGaugeOpen) -> String {
    format!(
        "attack gauge: opened number {serial} on your copy of {} like the host's ({} nodes, {:.1} of {:.1} s)",
        actors_text(open.boss, open.partner),
        open.nodes.len(),
        open.remaining_seconds,
        open.total_seconds
    )
}

pub fn guest_open_readback_text(serial: u32, reading: Option<&GaugeReading>) -> String {
    let Some(reading) = reading else {
        return format!("attack gauge: after opening number {serial} the gauge reads closed");
    };
    let nodes: Vec<AttackGaugeNode> = reading.nodes.iter().map(|node| node.node).collect();
    format!(
        "attack gauge: after opening number {serial}: mode {}, dummy {:#x}, nodes {} (hp {}), {:.1} of {:.1} s",
        reading.mode,
        reading.dummy,
        nodes.len(),
        hp_list(&nodes),
        reading.remaining_seconds,
        reading.total_seconds
    )
}

pub fn guest_close_text(serial: Option<u32>, step: GuestGaugeStep) -> String {
    let reason = match step {
        GuestGaugeStep::CloseHostClosed => "the host's closed",
        GuestGaugeStep::CloseOtherOpening => "the host shows another opening",
        GuestGaugeStep::CloseHostHasNone => "the host has none",
        _ => "asked",
    };
    serial.map_or_else(
        || format!("attack gauge: closed your own gauge ({reason})"),
        |serial| format!("attack gauge: closed number {serial} ({reason})"),
    )
}

pub fn guest_wait_text(serial: Option<u32>, wait: GuestGaugeWait) -> String {
    let reason = match wait {
        GuestGaugeWait::BossCopyMissing => "your copy of the boss is not here yet",
        GuestGaugeWait::PartnerCopyMissing => "your copy of the second actor is not here yet",
        GuestGaugeWait::GuestOnlyGrace => "your own gauge opened, waiting for the host's",
        GuestGaugeWait::HostAsked => {
            "your own gauge opened on a shared boss, the host was asked to open it"
        }
    };
    serial.map_or_else(
        || format!("attack gauge: waiting ({reason})"),
        |serial| format!("attack gauge: waiting with number {serial} ({reason})"),
    )
}

pub fn own_gauge_hit_text(node: u8, damage: i32, seq: u32, serial: u32) -> String {
    format!(
        "attack gauge: your hit on node {node} took {damage}, sent to the host as number {seq} for opening {serial}"
    )
}

pub fn guest_ask_sent_text(ask: &AttackGaugeAsk) -> String {
    format!(
        "attack gauge: your game opened a gauge on {} the host does not have, asked the host to open it (ask {}, {} nodes, hp {}, {:.1} of {:.1} s)",
        actors_text(ask.open.boss, ask.open.partner),
        ask.ask,
        ask.open.nodes.len(),
        hp_list(&ask.open.nodes),
        ask.open.remaining_seconds,
        ask.open.total_seconds
    )
}

pub fn host_ask_refused_text(ask: &AttackGaugeAsk, refusal: GaugeAskRefusal) -> String {
    let reason = match refusal {
        GaugeAskRefusal::SwitchedOff => "gauge sync is switched off",
        GaugeAskRefusal::OtherPhase => "another phase",
        GaugeAskRefusal::Repeated => "already answered",
        GaugeAskRefusal::HostGaugeOpen => "your own gauge is open, the guest follows it",
        GaugeAskRefusal::JustClosed => "your gauge closed moments ago",
        GaugeAskRefusal::BossMissing => "the boss is not here",
        GaugeAskRefusal::PartnerMissing => "the second actor is not here",
    };
    format!(
        "attack gauge: guest ask {} to open a gauge on {} refused ({reason})",
        ask.ask,
        actors_text(ask.open.boss, ask.open.partner)
    )
}

pub fn host_ask_replayed_text(
    ask: &AttackGaugeAsk,
    replay: &OpeningHitReplay,
    report: &OpeningReplayReport,
) -> String {
    let (kind, class) = unpack_hit_kind(replay.hit.attack_kind);
    format!(
        "attack gauge: guest ask {} on {}: replayed the guest's hit id {} as your player's hit (flags {:#x}, kind {kind}, class {}, hp {}, {} ms old), boss hp {} -> {} {}; your game opened its own gauge: {}; {}; boss vtable {}",
        ask.ask,
        actors_text(ask.open.boss, ask.open.partner),
        replay.hit.hit_id,
        replay.hit.flags,
        class.map_or_else(|| "unsent".to_owned(), |class| class.to_string()),
        replay.hit.hp_damage,
        replay.age_ms,
        report.hp_before,
        report.hp_after_hit,
        if report.hp_put_back {
            "and put back"
        } else {
            "NOT put back"
        },
        if report.gauge_opened { "yes" } else { "no" },
        boss_gauge_state_text(report.boss_state),
        report
            .boss_vtable_rva
            .map_or_else(|| "unreadable".to_owned(), |rva| format!("{rva:#x}"))
    )
}

pub fn host_ask_gave_up_text(ask: &AttackGaugeAsk, miss: OpeningHitMiss) -> String {
    let reason = match miss {
        OpeningHitMiss::NoGuestHit => "no guest hit on a boss arrived",
        OpeningHitMiss::OtherBoss => "the guest's last hit was on another enemy",
        OpeningHitMiss::TooOld => "the guest's last hit on it is too old",
        OpeningHitMiss::BossNearlyDead => "the boss has 1 hp or less",
    };
    format!(
        "attack gauge: guest ask {} on {}: no hit replayed ({reason}), your game opens no gauge for it",
        ask.ask,
        actors_text(ask.open.boss, ask.open.partner)
    )
}

pub fn node_break_text(
    node: u8,
    before: Option<BossGaugeState>,
    closed: bool,
    after: Option<BossGaugeState>,
) -> String {
    let refused = before.is_some_and(|state| !state.takes_a_break()) && !closed;
    format!(
        "attack gauge: ran the game's node break for node {node} ({} before), the gauge now reads {} ({} after){}",
        boss_gauge_state_text(before),
        if closed { "closed" } else { "open" },
        boss_gauge_state_text(after),
        if refused {
            ", the boss refused the break: its script did not open this gauge"
        } else {
            ""
        }
    )
}

/// The once-a-second line while a gauge is open: `side` is "host" or "guest".
pub fn gauge_tally_text(
    side: &str,
    serial: Option<u32>,
    nodes: &[AttackGaugeNode],
    tally: &GaugeTally,
) -> String {
    let opening = serial.map_or_else(
        || "own gauge".to_owned(),
        |serial| format!("number {serial}"),
    );
    let traded = if side == "host" {
        format!(
            "guest hits received {}, applied {}",
            tally.sent, tally.applied
        )
    } else {
        format!(
            "your hits sent {}, held for the host {}",
            tally.sent, tally.held
        )
    };
    let why = tally.last_miss.map_or("none", |miss| match miss {
        GaugeSwingMiss::OtherDummy => "the hit was on another dummy",
        GaugeSwingMiss::GameTookNothing => {
            "the game's hit function took nothing (menu open, or not your player's hit)"
        }
        GaugeSwingMiss::SameSwing => "the same swing hit the node again",
        GaugeSwingMiss::NotConnected => "no partner connection",
        GaugeSwingMiss::DroppedByHost => "dropped by the host",
    });
    format!(
        "attack gauge: {side} {opening}: nodes hp {}; {traded}; not counted {} (last: {why})",
        hp_list(nodes),
        tally.not_counted
    )
}

fn actors_text(boss: EnemyId, partner: Option<EnemyId>) -> String {
    let id = |id: EnemyId| format!("{}:{}", id.group, id.ordinal);
    partner.map_or_else(
        || id(boss),
        |partner| format!("{} and {}", id(boss), id(partner)),
    )
}

fn hp_list(nodes: &[AttackGaugeNode]) -> String {
    let parts: Vec<String> = nodes
        .iter()
        .map(|node| format!("{}/{}", node.hp, node.max_hp))
        .collect();
    parts.join(", ")
}
