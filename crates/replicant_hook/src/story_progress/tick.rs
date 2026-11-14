use std::sync::{LazyLock, Mutex, PoisonError};

use replicant_game::downed::OWN_DRIFT_HOLD_AFTER_REWIND_MS;
use replicant_game::fix_buttons::TakeOnce;
use replicant_game::stay_together::FollowRole;
use replicant_game::story_progress::{
    FeedSend, SAME_STORY_REQUEST_IGNORED_MS, STORY_DIGEST_INTERVAL_MS,
    STORY_MESSAGES_KEPT_BEFORE_ARMING, STORY_REQUEST_RETRY_MS, StoryArrays, StoryFeed, StoryMirror,
};
use replicant_game::story_writes::STORY_WRITE_HOLD_MS;
use replicant_proto::{
    EndingRecords, GameReadiness, Message, StoryArray, StoryChange, StoryDigest, StoryRequest,
};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::net::NetSharedExt;
use crate::net::{self, NetShared, NetState, NetThread, net_clock_ms};
use crate::story_progress::calls::{
    apply_change, read_live_arrays, read_live_story, read_loaded_slot,
};
use crate::story_progress::ending_records;
use crate::story_progress::save_rule::{SnapshotKey, arm, armed_key};

const LOGGED_IDS_PER_LINE: usize = 8;

static RESYNC_REQUESTED: TakeOnce = TakeOnce::new();
static HOST: LazyLock<Mutex<HostStory>> = LazyLock::new(Mutex::default);
static GUEST: LazyLock<Mutex<GuestStory>> = LazyLock::new(Mutex::default);

#[derive(Default)]
struct HostStory {
    feed: StoryFeed,
    been_in_play: bool,
    request_waiting: bool,
    last_whole_sent_ms: Option<u64>,
    last_digest_ms: u64,
    last_records_sent: Option<EndingRecords>,
}

#[derive(Default)]
struct GuestStory {
    mirror: StoryMirror,
    requested_at_ms: Option<u64>,
    queued: Vec<Message>,
}

pub fn request_story_resync() {
    RESYNC_REQUESTED.request();
}

pub fn story_digest() -> Option<u64> {
    let host = HOST
        .try_lock()
        .ok()
        .and_then(|host| host.feed.sent_digest());
    host.or_else(|| {
        GUEST
            .try_lock()
            .ok()
            .and_then(|guest| guest.mirror.host_digest())
    })
}

pub fn tick(build: &VerifiedBuild) {
    let Some(net) = net::handle() else {
        RESYNC_REQUESTED.clear();
        return;
    };
    let view = net.view();
    let arrived = net.take_relayed(|message| {
        matches!(
            message,
            Message::StoryRequest(_)
                | Message::StoryPart(_)
                | Message::StoryChanges(_)
                | Message::StoryDigest(_)
                | Message::EndingRecords(_)
        )
    });
    let now = net_clock_ms();
    match FollowRole::from_slot(view.local_slot) {
        FollowRole::Host => {
            RESYNC_REQUESTED.clear();
            host_tick(build, net, &view, arrived, now);
        }
        FollowRole::Guest => guest_tick(build, net, &view, arrived, now),
        FollowRole::Unknown => RESYNC_REQUESTED.clear(),
    }
}

fn host_tick(
    build: &VerifiedBuild,
    net: &NetThread,
    view: &NetShared,
    arrived: Vec<Message>,
    now: u64,
) {
    let mut host = HOST.lock().unwrap_or_else(PoisonError::into_inner);
    if view.state != NetState::Connected {
        if host.feed.is_active() {
            log_line("story: guest left, host stops sending story changes");
        }
        host.feed.reset();
        crate::containers::clear_held_flags();
        host.request_waiting = false;
        host.last_whole_sent_ms = None;
        host.last_records_sent = None;
        return;
    }
    host.been_in_play |= view.effective_local_readiness().is_in_play();
    if arrived
        .iter()
        .any(|message| matches!(message, Message::StoryRequest(_)))
    {
        let just_answered = host
            .last_whole_sent_ms
            .is_some_and(|sent| now.saturating_sub(sent) < SAME_STORY_REQUEST_IGNORED_MS);
        if just_answered {
            log_line("story: host ignored a request, it just sent its whole story state");
        } else {
            host.request_waiting = true;
        }
    }
    if !host.been_in_play {
        return;
    }
    let force_records = host.request_waiting;
    ending_records::host_send(build, net, &mut host.last_records_sent, force_records);
    let Some(current) = read_live_arrays(build) else {
        return;
    };
    if host.request_waiting {
        host.request_waiting = false;
        for part in host.feed.whole(&current) {
            net.send_relayed(Message::StoryPart(part));
        }
        host.last_digest_ms = now;
        host.last_whole_sent_ms = Some(now);
        log_line(&format!(
            "story: host sent its whole story state ({} game flags, {} snow flags set)",
            current.set_flag_count(StoryArray::GameFlags),
            current.set_flag_count(StoryArray::SnowFlags)
        ));
        return;
    }
    match host.feed.next(&current) {
        FeedSend::Nothing => {}
        FeedSend::Changes(batches) => {
            crate::story_writes::note_host_story_changes(host.feed.last_changes());
            let changes: Vec<StoryChange> = batches
                .iter()
                .flat_map(|batch| batch.changes.iter().copied())
                .collect();
            log_line(&format!(
                "story: host sent {} changes: {}",
                changes.len(),
                describe_changes(&changes)
            ));
            for batch in batches {
                net.send_relayed(Message::StoryChanges(batch));
            }
        }
        FeedSend::WholeArrays(parts) => {
            crate::story_writes::note_host_story_changes(host.feed.last_changes());
            log_line("story: host's story state changed in bulk, sent it whole");
            for part in parts {
                net.send_relayed(Message::StoryPart(part));
            }
        }
    }
    if now.saturating_sub(host.last_digest_ms) >= STORY_DIGEST_INTERVAL_MS
        && let Some(hash) = host.feed.sent_digest()
    {
        host.last_digest_ms = now;
        net.send_relayed(Message::StoryDigest(StoryDigest { hash }));
    }
}

/// Reads the guest's copy of the host's story state, once the whole of it has arrived.
pub(crate) fn with_host_story<R>(read: impl FnOnce(&StoryArrays) -> R) -> Option<R> {
    GUEST
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .mirror
        .host()
        .map(read)
}

fn guest_tick(
    build: &VerifiedBuild,
    net: &NetThread,
    view: &NetShared,
    arrived: Vec<Message>,
    now: u64,
) {
    let mut guest = GUEST.lock().unwrap_or_else(PoisonError::into_inner);
    if view.state != NetState::Connected {
        if guest.mirror.is_complete() || guest.requested_at_ms.is_some() {
            log_line("story: host left; the guest keeps the last synced story in memory");
        }
        guest.mirror.clear();
        guest.requested_at_ms = None;
        guest.queued.clear();
        RESYNC_REQUESTED.clear();
        crate::containers::clear_held_flags();
        ending_records::end_session(build);
        return;
    }
    if RESYNC_REQUESTED.take() {
        guest.mirror.clear();
        guest.requested_at_ms = None;
        log_line("story: guest dropped its copy of the host's story on request, asking again");
    }
    let readiness = view.effective_local_readiness();
    guest.queued.extend(arrived);
    if guest.queued.len() > STORY_MESSAGES_KEPT_BEFORE_ARMING {
        let surplus = guest.queued.len() - STORY_MESSAGES_KEPT_BEFORE_ARMING;
        guest.queued.drain(..surplus);
        log_line(&format!(
            "story: {surplus} waiting story messages dropped, too many before the join snapshot"
        ));
    }
    let mut slot_changed = false;
    if let Some((own, name)) = read_live_story(build) {
        let key = SnapshotKey {
            slot: read_loaded_slot(build),
            name,
        };
        let known = armed_key();
        if known != Some(key) {
            if readiness.is_in_play() {
                arm(own, key);
            } else {
                slot_changed = known.is_some();
            }
        }
    }
    crate::containers::expire_held_flags(guest.mirror.host());
    if readiness.is_in_play() {
        ending_records::arm_once(build);
    }
    if armed_key().is_none() {
        return;
    }
    let arrived = std::mem::take(&mut guest.queued);
    let together = readiness.is_in_play() && view.partner_in_play() && same_phase(view);
    for message in arrived {
        match message {
            Message::EndingRecords(records) => ending_records::receive_host(records, together),
            Message::StoryPart(part) => {
                guest.mirror.receive_part(&part);
                if guest.mirror.is_complete() {
                    guest.requested_at_ms = None;
                }
            }
            Message::StoryChanges(batch) => {
                if !guest.mirror.receive_changes(&batch.changes) {
                    log_line(&format!(
                        "story: {} changes came before the whole state, dropped",
                        batch.changes.len()
                    ));
                }
            }
            Message::StoryDigest(digest) => {
                if let Some(mine) = guest.mirror.host_digest()
                    && mine != digest.hash
                {
                    log_line("story: guest's copy of the host's story differs, asking again");
                    guest.mirror.clear();
                    guest.requested_at_ms = None;
                }
            }
            _ => {}
        }
    }
    ending_records::sync_live(build);
    if !guest.mirror.is_complete()
        && guest
            .requested_at_ms
            .is_none_or(|at| now.saturating_sub(at) >= STORY_REQUEST_RETRY_MS)
    {
        guest.requested_at_ms = Some(now);
        net.send_relayed(Message::StoryRequest(StoryRequest));
        log_line("story: guest asked the host for its whole story state");
    }
    if readiness == GameReadiness::NotLinked || slot_changed {
        return;
    }
    let Some(live) = read_live_arrays(build) else {
        return;
    };
    for key in crate::story_writes::take_holds() {
        guest.mirror.hold(key, now + STORY_WRITE_HOLD_MS);
    }
    if crate::rewind::take_rewound_notice() {
        guest
            .mirror
            .hold_own_drift_until(now + OWN_DRIFT_HOLD_AFTER_REWIND_MS);
        log_line("story: guest holds its own changes while the host's rewound story arrives");
    }
    let mut plan = guest.mirror.plan(&live, now);
    plan.fixes = crate::containers::keep_held_flags(plan.fixes, guest.mirror.host());
    if plan.fixes.is_empty() {
        return;
    }
    for &change in &plan.fixes {
        apply_change(build, change);
    }
    crate::story_writes::note_mirror_fixes_applied(&plan.fixes);
    log_line(&format!(
        "story: guest applied {} changes{} ({} own changes still held): {}",
        plan.fixes.len(),
        if plan.reload_restored {
            " after its story was replaced by a load"
        } else {
            ""
        },
        plan.held,
        describe_changes(&plan.fixes)
    ));
}

fn same_phase(view: &NetShared) -> bool {
    match (&view.local, view.partner_state()) {
        (Some(local), Some(remote)) => local.phase == remote.state.phase,
        _ => false,
    }
}

pub(crate) fn describe_changes(changes: &[StoryChange]) -> String {
    let mut parts: Vec<String> = changes
        .iter()
        .take(LOGGED_IDS_PER_LINE)
        .map(|change| match *change {
            StoryChange::Flag { array, id, on } => {
                format!("{}{id}{}", short_name(array), if on { "+" } else { "-" })
            }
            StoryChange::Counter { array, id, value } => {
                format!("{}{id}={value}", short_name(array))
            }
        })
        .collect();
    if changes.len() > LOGGED_IDS_PER_LINE {
        parts.push(format!("and {} more", changes.len() - LOGGED_IDS_PER_LINE));
    }
    parts.join(" ")
}

fn short_name(array: StoryArray) -> &'static str {
    match array {
        StoryArray::GameFlags => "g",
        StoryArray::SnowFlags => "s",
        StoryArray::GameCounters => "gc",
        StoryArray::SnowCounters => "sc",
        StoryArray::KeyFragments => "key",
    }
}
