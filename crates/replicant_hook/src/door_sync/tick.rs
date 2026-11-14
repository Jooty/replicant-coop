use std::sync::{LazyLock, Mutex, OnceLock, PoisonError};
use std::time::Instant;

use replicant_game::PHASE_TABLE;
use replicant_game::door_sync::{
    PendingDoorStates, SnapshotSchedule, changes_for_resulting_state, door_reached_state,
    message_of_action, plan_door_state, sender_phase_matches_local,
};
use replicant_game::fight_doors::{
    FightLockVerdict, LockOrigin, fight_door_in_zone, judge_door_lock,
};
use replicant_game::stay_together::FollowRole;
use replicant_proto::{
    DoorAction, DoorChange, DoorReport, DoorState, MAX_DOORS_PER_REPORT, Message,
};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::door_sync::calls::{door_of_layout_id, read_door_state, send_door_message};
use crate::door_sync::fight_lock::{HELD_LOCKS, log_fight_lock};
use crate::door_sync::observe::{GUEST_USES, REGISTRY};
use crate::net::NetSharedExt;
use crate::net::{self, NetThread};

static STARTED_AT: OnceLock<Instant> = OnceLock::new();
static SCHEDULE: Mutex<SnapshotSchedule> = Mutex::new(SnapshotSchedule::new());
static PENDING: LazyLock<Mutex<PendingDoorStates>> = LazyLock::new(Mutex::default);
static LAST_HOST_PHASE: Mutex<Option<u16>> = Mutex::new(None);

pub(super) fn now_ms() -> u64 {
    u64::try_from(STARTED_AT.get_or_init(Instant::now).elapsed().as_millis()).unwrap_or(u64::MAX)
}

pub fn tick(build: &VerifiedBuild) {
    let Some(net) = net::handle() else {
        return;
    };
    let view = net.view();
    let role = FollowRole::from_slot(view.local_slot);
    let local_phase = view.local.as_ref().map(|local| local.phase);
    forget_doors_when_the_zone_changes(local_phase);
    let arrived = net.take_relayed(|message| {
        matches!(
            message,
            Message::DoorChange(_) | Message::DoorRequest(_) | Message::DoorReport(_)
        )
    });
    let now = now_ms();
    match role {
        FollowRole::Host => {
            let partner_phase = view
                .partner_in_play()
                .then(|| view.partner_state().map(|remote| remote.state.phase))
                .flatten();
            host_tick(build, net, arrived, (local_phase, partner_phase), now);
        }
        FollowRole::Guest => guest_tick(build, arrived, local_phase, now),
        FollowRole::Unknown => {}
    }
}

fn forget_doors_when_the_zone_changes(local_phase: Option<u16>) {
    let mut last = LAST_HOST_PHASE
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if *last == local_phase {
        return;
    }
    *last = local_phase;
    REGISTRY
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    GUEST_USES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    HELD_LOCKS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    log_line(&format!(
        "doors: zone is now {local_phase:?}, door list cleared"
    ));
}

fn host_tick(
    build: &VerifiedBuild,
    net: &NetThread,
    arrived: Vec<Message>,
    (local_phase, partner_phase): (Option<u16>, Option<u16>),
    now: u64,
) {
    for message in arrived {
        if let Message::DoorRequest(change) = message
            && apply_change(build, change, local_phase, "host opened for the guest")
        {
            echo_resulting_state(build, net, change, local_phase);
        }
    }
    let due = {
        let mut schedule = SCHEDULE.lock().unwrap_or_else(PoisonError::into_inner);
        schedule.observe(local_phase, partner_phase, now);
        schedule.take_due(now)
    };
    if let Some(phase) = due {
        send_door_states(build, net, phase);
    }
}

fn guest_tick(build: &VerifiedBuild, arrived: Vec<Message>, local_phase: Option<u16>, now: u64) {
    let mut pending = PENDING.lock().unwrap_or_else(PoisonError::into_inner);
    for message in arrived {
        match message {
            Message::DoorChange(change) => {
                mirror_host_change(build, change, local_phase);
            }
            Message::DoorReport(report) => {
                log_line(&format!(
                    "doors: guest got {} door states for zone {}",
                    report.doors.len(),
                    report.phase
                ));
                pending.receive(report.phase, &report.doors, now);
            }
            _ => {}
        }
    }
    let outcome = pending.retry(local_phase, now, |state| {
        apply_state(build, state, local_phase)
    });
    if outcome.applied > 0 || outcome.given_up > 0 {
        log_line(&format!(
            "doors: guest applied {} door states, gave up on {}",
            outcome.applied, outcome.given_up
        ));
    }
}

fn mirror_host_change(build: &VerifiedBuild, change: DoorChange, local_phase: Option<u16>) {
    let locking = change.action == DoorAction::Lock && change.flag;
    if change.action == DoorAction::Lock && !change.flag {
        HELD_LOCKS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drop_hold(change.layout_id);
    }
    if locking && fight_door_in_zone(change.phase, change.layout_id) {
        let phase = PHASE_TABLE
            .get(usize::from(change.phase))
            .map_or("", |entry| entry.name);
        let verdict = judge_door_lock(
            phase,
            i32::from(change.layout_id),
            true,
            LockOrigin::MirroredFromHost,
        );
        log_fight_lock(phase, change.layout_id, verdict, "the host");
        if let FightLockVerdict::Hold(_) = verdict {
            HELD_LOCKS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .hold(change.layout_id);
            return;
        }
    }
    apply_change(build, change, local_phase, "guest mirrors the host");
}

fn apply_change(
    build: &VerifiedBuild,
    change: DoorChange,
    local_phase: Option<u16>,
    who: &str,
) -> bool {
    if !sender_phase_matches_local(change.phase, local_phase) {
        log_line(&format!(
            "doors: {who}: door {} {:?} {} is for zone {}, this zone is {local_phase:?}, dropped",
            change.layout_id, change.action, change.flag, change.phase
        ));
        return false;
    }
    let Some(door) = door_of_layout_id(build, change.layout_id) else {
        log_line(&format!(
            "doors: {who}: door {} is not in this zone, {:?} {} dropped",
            change.layout_id, change.action, change.flag
        ));
        return false;
    };
    send_door_message(
        build,
        door,
        message_of_action(change.action),
        i64::from(change.flag),
    );
    log_line(&format!(
        "doors: {who}: door {} got {:?} {}",
        change.layout_id, change.action, change.flag
    ));
    true
}

fn echo_resulting_state(
    build: &VerifiedBuild,
    net: &NetThread,
    request: DoorChange,
    local_phase: Option<u16>,
) {
    let Some(phase) = local_phase else {
        return;
    };
    let Some((open, locked)) =
        door_of_layout_id(build, request.layout_id).and_then(|door| read_door_state(build, door))
    else {
        return;
    };
    let state = DoorState {
        layout_id: request.layout_id,
        open,
        locked,
    };
    for change in changes_for_resulting_state(phase, &state) {
        net.send_relayed(Message::DoorChange(change));
    }
    log_line(&format!(
        "doors: host tells everyone door {} is now open {open} locked {locked}",
        request.layout_id
    ));
}

fn apply_state(build: &VerifiedBuild, state: &DoorState, local_phase: Option<u16>) -> bool {
    let Some(door) = door_of_layout_id(build, state.layout_id) else {
        return false;
    };
    let Some((open, locked)) = read_door_state(build, door) else {
        return false;
    };
    let keeps_its_own_fight_lock =
        state.locked && local_phase.is_some_and(|phase| fight_door_in_zone(phase, state.layout_id));
    let state = &DoorState {
        locked: if keeps_its_own_fight_lock {
            locked
        } else {
            state.locked
        },
        ..*state
    };
    for (action, flag) in plan_door_state(open, locked, state) {
        send_door_message(build, door, message_of_action(action), i64::from(flag));
    }
    let reached = read_door_state(build, door)
        .is_some_and(|(now_open, now_locked)| door_reached_state(now_open, now_locked, state));
    log_line(&format!(
        "doors: guest set door {} to open {} locked {} (was open {open} locked {locked}), reached {reached}",
        state.layout_id, state.open, state.locked
    ));
    reached
}

fn send_door_states(build: &VerifiedBuild, net: &NetThread, phase: u16) {
    let ids = REGISTRY
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .ids();
    let doors: Vec<DoorState> = ids
        .into_iter()
        .filter_map(|layout_id| {
            let door = door_of_layout_id(build, layout_id)?;
            let (open, locked) = read_door_state(build, door)?;
            Some(DoorState {
                layout_id,
                open,
                locked,
            })
        })
        .collect();
    log_line(&format!(
        "doors: host sends {} door states for zone {phase}",
        doors.len()
    ));
    for chunk in doors.chunks(MAX_DOORS_PER_REPORT) {
        net.send_relayed(Message::DoorReport(DoorReport {
            phase,
            doors: chunk.to_vec(),
        }));
    }
}
