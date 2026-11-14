use std::cell::OnceCell;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use replicant_game::cutscene_skip::SKIP_PROMPT_UI_RVA as HUD_OBJECT_RVA;
use replicant_game::enemies::{AttackRecord, read_boss_bar_open, read_group_count};
use replicant_game::kaine::{
    COMPANION_GAUGE_MODE_OFFSET, COMPANION_LENT_AFTER_A_CALL_MS, COMPANION_LENT_AFTER_A_SCENE_MS,
    CompanionLendReason, CompanionOff, CompanionOffBossSigns, CompanionOffEvent, CompanionOffStep,
    CompanionOffTick, HUD_PARTNER_GAUGE_OFFSET, PARTNER_GAUGE_ACTOR_HANDLE_OFFSET, StandInSide,
};
use replicant_game::marker::read_world_flags;
use replicant_game::offsets::{
    ACTOR_SELF_HANDLE_OFFSET, PARTNER_ACTIVE_FLAG_OFFSET, PARTNER_KAINE_ACTOR_RVA,
};
use replicant_game::story_progress::StoryArrays;
use replicant_game::{GameMemory, Replicant};
use replicant_proto::StoryArray;

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::kaine::companion_rows_from_scripts;
use crate::kaine::stand_in::session_side;
use crate::memory::InProcessMemory;
use crate::net::net_clock_ms;
use crate::story_progress::{read_live_arrays, with_host_story};

const COMPANION_OFF_LINE_INTERVAL: Duration = Duration::from_secs(1);

static RULE: Mutex<CompanionOff> = Mutex::new(CompanionOff::new());
static WINDOW_OPEN: AtomicBool = AtomicBool::new(false);
static GUEST_IN_SESSION: AtomicBool = AtomicBool::new(false);
static HITS_SHE_LANDED: AtomicU32 = AtomicU32::new(0);
static LAST_LINE: Mutex<Option<Instant>> = Mutex::new(None);

/// A script function ran on this game (popped or traced): it may open or close a fight window
/// in which the companion is kept off.
pub(crate) fn companion_off_function_ran(build: &VerifiedBuild, name: &str) {
    let mut memory = InProcessMemory::new(build);
    let pack = Replicant::new(InProcessMemory::new(build))
        .current_phase()
        .ok();
    let mut rule = RULE.lock().unwrap_or_else(PoisonError::into_inner);
    log_event(rule.use_rows_from_scripts(companion_rows_from_scripts()));
    if let Some(pack) = pack {
        log_event(rule.enter_pack(&pack));
    }
    let host_story = OnceCell::new();
    log_event(rule.function_ran_in_story(name, |id| {
        host_story
            .get_or_init(|| story_flags_of_the_host(build))
            .as_ref()
            .is_some_and(|arrays| arrays.flag(StoryArray::GameFlags, id))
    }));
    WINDOW_OPEN.store(rule.open_fight().is_some(), Ordering::Relaxed);
    let actor = memory.module_base() + u64::from(PARTNER_KAINE_ACTOR_RVA);
    let Ok(found) = memory.read_u8(actor + PARTNER_ACTIVE_FLAG_OFFSET) else {
        return;
    };
    let tick = CompanionOffTick {
        guest_in_session: GUEST_IN_SESSION.load(Ordering::Relaxed),
        active_now: found != 0,
        event_scene: read_world_flags(&memory).is_ok_and(|flags| flags.event_scene),
        now_ms: net_clock_ms(),
    };
    let Some(lend) = rule.function_addresses_her(name, tick) else {
        return;
    };
    drop(rule);
    let written = lend.write_active
        && found == 0
        && memory
            .write_u8(actor + PARTNER_ACTIVE_FLAG_OFFSET, 1)
            .is_ok();
    log_line(&format!(
        "companion off ({}): {name} addresses her, she is given back for it and the {COMPANION_LENT_AFTER_A_CALL_MS} ms after (active byte found {found}, {})",
        lend.fight,
        if written {
            "written 1"
        } else {
            "left as found"
        }
    ));
}

/// The fight whose companion-off window is open on this game, if any.
pub(crate) fn companion_off_fight() -> Option<&'static str> {
    RULE.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .open_fight()
}

/// Keeps the companion inactive (her active byte 0, what `_DisactivateUniqueActor(-2)` writes)
/// while a listed fight window is open and a guest is in the session, except while she is lent
/// to a scene or a call that addresses her; called once per ghost tick.
pub(crate) fn keep_companion_off(build: &VerifiedBuild, guest_in_session: bool) {
    GUEST_IN_SESSION.store(guest_in_session, Ordering::Relaxed);
    let mut memory = InProcessMemory::new(build);
    let actor = memory.module_base() + u64::from(PARTNER_KAINE_ACTOR_RVA);
    let mut rule = RULE.lock().unwrap_or_else(PoisonError::into_inner);
    log_event(rule.use_rows_from_scripts(companion_rows_from_scripts()));
    if let Ok(pack) = Replicant::new(InProcessMemory::new(build)).current_phase() {
        log_event(rule.enter_pack(&pack));
    }
    if rule.story_flags_are_read_here()
        && let Some(arrays) = story_flags_of_the_host(build)
    {
        log_event(rule.story_flags(|id| arrays.flag(StoryArray::GameFlags, id)));
    }
    let event_scene = read_world_flags(&memory).is_ok_and(|flags| flags.event_scene);
    let now_ms = net_clock_ms();
    if let Some(group) = rule.boss_group_watched() {
        log_event(rule.boss_signs(CompanionOffBossSigns {
            gauge_open: read_boss_bar_open(&memory),
            boss_actors: read_group_count(&memory, group),
            event_scene,
            now_ms,
        }));
    }
    WINDOW_OPEN.store(rule.open_fight().is_some(), Ordering::Relaxed);
    let Ok(found) = memory.read_u8(actor + PARTNER_ACTIVE_FLAG_OFFSET) else {
        return;
    };
    let step = rule.step(CompanionOffTick {
        guest_in_session,
        active_now: found != 0,
        event_scene,
        now_ms,
    });
    let fight = rule.open_fight().unwrap_or("no fight");
    match step {
        CompanionOffStep::Deactivate => {
            if memory
                .write_u8(actor + PARTNER_ACTIVE_FLAG_OFFSET, 0)
                .is_err()
            {
                log_line("companion off: cannot write her active byte");
            }
        }
        CompanionOffStep::GiveBack { active } => {
            if active {
                let _ = memory.write_u8(actor + PARTNER_ACTIVE_FLAG_OFFSET, 1);
            }
            log_line(&format!(
                "companion off: given back to the scripts as they left her (active byte {})",
                u8::from(active)
            ));
        }
        CompanionOffStep::Lend { write_active, why } => {
            let written = write_active
                && memory
                    .write_u8(actor + PARTNER_ACTIVE_FLAG_OFFSET, 1)
                    .is_ok();
            log_line(&format!(
                "companion off ({fight}): she is given back for {}, active byte found {found}, {}",
                lend_text(&why),
                if written {
                    "written 1"
                } else {
                    "left as found"
                }
            ));
        }
        CompanionOffStep::LendOver { deactivate, why } => {
            if deactivate {
                let _ = memory.write_u8(actor + PARTNER_ACTIVE_FLAG_OFFSET, 0);
            }
            log_line(&format!(
                "companion off ({fight}): the lend for {} is over, active byte found {found}, {}",
                lend_text(&why),
                if deactivate {
                    "she is off again"
                } else {
                    "the scripts left her off"
                }
            ));
        }
        CompanionOffStep::Leave => {}
    }
    if !line_is_due() {
        return;
    }
    let Some(report) = rule.take_report() else {
        return;
    };
    let watched_group = rule.boss_group_watched();
    drop(rule);
    let left = memory
        .read_u8(actor + PARTNER_ACTIVE_FLAG_OFFSET)
        .map_or_else(|_| "unreadable".to_owned(), |byte| byte.to_string());
    let turned_on_by = if report.turned_on_by.is_empty() {
        "nobody".to_owned()
    } else {
        report.turned_on_by.join(", ")
    };
    let lent = report
        .lent
        .as_ref()
        .map_or_else(|| "no".to_owned(), lend_text);
    log_line(&format!(
        "companion off ({}): guest in session {guest_in_session}, active byte found {found} left {left}, \
         turned on since the last line by {turned_on_by}, lent to the game: {lent}, event scene {event_scene}, \
         hits she landed {} (must be 0 unless lent), {}{}",
        report.fight,
        HITS_SHE_LANDED.swap(0, Ordering::Relaxed),
        gauge_text(&memory, actor),
        boss_signs_text(&memory, watched_group)
    ));
}

/// Called for every hit the game hands a victim: counts the hits the companion lands while a
/// window is open, for the once-a-second line.
pub(crate) fn note_hit_for_companion_off(build: &VerifiedBuild, victim: u64, record: u64) {
    if !WINDOW_OPEN.load(Ordering::Relaxed) {
        return;
    }
    let memory = InProcessMemory::new(build);
    let her = memory.module_base() + u64::from(PARTNER_KAINE_ACTOR_RVA);
    if victim == her {
        return;
    }
    let Some(attack) = AttackRecord::read(&memory, record) else {
        return;
    };
    if attack.sources(&memory).contains(&Some(her)) {
        HITS_SHE_LANDED.fetch_add(1, Ordering::Relaxed);
    }
}

fn line_is_due() -> bool {
    let mut last = LAST_LINE.lock().unwrap_or_else(PoisonError::into_inner);
    let now = Instant::now();
    if last.is_some_and(|at| now.saturating_duration_since(at) < COMPANION_OFF_LINE_INTERVAL) {
        return false;
    }
    *last = Some(now);
    true
}

fn gauge_text(memory: &InProcessMemory, actor: u64) -> String {
    let mode = memory
        .read_i32(actor + COMPANION_GAUGE_MODE_OFFSET)
        .map_or_else(|_| "unreadable".to_owned(), |mode| mode.to_string());
    let gauge_handle = memory.module_base()
        + u64::from(HUD_OBJECT_RVA)
        + HUD_PARTNER_GAUGE_OFFSET
        + PARTNER_GAUGE_ACTOR_HANDLE_OFFSET;
    let holds_her = match (
        memory.read_u64(gauge_handle),
        memory.read_u64(actor + ACTOR_SELF_HANDLE_OFFSET),
    ) {
        (Ok(gauge), Ok(hers)) => (gauge == hers).to_string(),
        _ => "unreadable".to_owned(),
    };
    // mode non-zero and her handle in the slot is how _SetKaineMode puts her gauge up; whether
    // the HUD still draws it while she is inactive is not known
    format!("life gauge: her mode {mode}, the HUD partner gauge holds her handle {holds_her}")
}

fn boss_signs_text(memory: &InProcessMemory, group: Option<u16>) -> String {
    let Some(group) = group else {
        return String::new();
    };
    let gauge =
        read_boss_bar_open(memory).map_or_else(|| "unreadable".to_owned(), |open| open.to_string());
    let actors = read_group_count(memory, group)
        .map_or_else(|| "unreadable".to_owned(), |count| count.to_string());
    format!(", boss signs: boss life gauge open {gauge}, actors in group {group} {actors}")
}

fn lend_text(why: &CompanionLendReason) -> String {
    match why {
        CompanionLendReason::Scene => {
            format!("the event scene and the {COMPANION_LENT_AFTER_A_SCENE_MS} ms after it")
        }
        CompanionLendReason::Call(name) => {
            format!("{name} and the {COMPANION_LENT_AFTER_A_CALL_MS} ms after it")
        }
    }
}

fn story_flags_of_the_host(build: &VerifiedBuild) -> Option<StoryArrays> {
    match session_side() {
        Some((StandInSide::Guest, _)) => with_host_story(StoryArrays::clone),
        _ => read_live_arrays(build),
    }
}

fn log_event(event: Option<CompanionOffEvent>) {
    match event {
        Some(CompanionOffEvent::Opened { fight, by }) => log_line(&format!(
            "companion off: {fight} window opened by {by}, she is kept inactive while a guest is in the session"
        )),
        Some(CompanionOffEvent::Closed { fight, by }) => {
            log_line(&format!("companion off: {fight} window closed by {by}"));
        }
        None => {}
    }
}
