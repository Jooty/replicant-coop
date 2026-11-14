use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use replicant_game::blocks::distance_between;
use replicant_game::kaine::{
    GUEST_HIT_SCALE_IN_A_STAND_IN_BEAT, GuestStandIn, HeldTrap, OwnTrapVerdict, STAND_IN_BEATS,
    StandInBeat, StandInEvent, StandInFacts, StandInSide, open_stand_in_beat, stand_in_hp_damage,
};
use replicant_game::marker::read_world_flags;
use replicant_game::stay_together::{FollowRole, local_phase_index};
use replicant_game::story_progress::StoryArrays;
use replicant_game::{Position, Replicant};
use replicant_proto::{CallbackFired, EnemyId, Message, StoryArray, StoryCall};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::kaine::companion_off::companion_off_fight;
use crate::memory::InProcessMemory;
use crate::net::{self, NetState, net_clock_ms};
use crate::story_progress::{read_live_arrays, with_host_story};
use crate::story_relay::{RelayPath, read_arguments, submit_relay_call};

const STAND_IN_LINE_INTERVAL: Duration = Duration::from_secs(1);

static STAND_IN: Mutex<GuestStandIn> = Mutex::new(GuestStandIn::new());
static LAST_LINE: Mutex<Option<Instant>> = Mutex::new(None);
static RELEASES_WAITING_FOR_THE_QUEUE: Mutex<Vec<StoryCall>> = Mutex::new(Vec::new());

/// A script function ran on this game (a pop that was let run, or a trace).
pub(crate) fn stand_in_function_ran(name: &str) {
    crate::kaine::scene_stand_in_function_ran(name);
    STAND_IN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .function_ran(name);
}

/// This game popped `name` itself. True when it must not run now: on the guest, a catch-up trap
/// of the open beat waits for the host's game. On the host it runs, and the guest is told.
pub(crate) fn hold_own_stand_in_trap(build: &VerifiedBuild, name: &str, arguments: u64) -> bool {
    let Some((side, in_session)) = session_side() else {
        return false;
    };
    let mut stand_in = STAND_IN.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(beat) = stand_in
        .open_beat()
        .filter(|beat| beat.held_traps.contains(&name))
    else {
        return false;
    };
    let args = if side == StandInSide::Guest {
        read_arguments(&InProcessMemory::new(build), arguments).unwrap_or_default()
    } else {
        Vec::new()
    };
    match stand_in.own_trap_fired(side, in_session, name, args, net_clock_ms()) {
        OwnTrapVerdict::Run => false,
        OwnTrapVerdict::Hold => {
            log_line(&format!(
                "stand-in ({}, beat {}): your game's own {name} is held for the host, not run here and not sent",
                beat.fight, beat.beat
            ));
            true
        }
        OwnTrapVerdict::RunAndTellTheGuest => {
            drop(stand_in);
            tell_the_guest(build, name, beat);
            false
        }
    }
}

/// The host's stand-in scale for a guest hit: `Some((beat, scaled hp damage))` while a beat is
/// open and `enemy` is its boss.
pub(crate) fn stand_in_hit_damage(enemy: EnemyId, hp_damage: u32) -> Option<(u8, u32)> {
    let mut stand_in = STAND_IN.lock().unwrap_or_else(PoisonError::into_inner);
    let beat = stand_in.scales_hits_on(enemy)?;
    stand_in.note_scaled_hit();
    Some((beat.beat, stand_in_hp_damage(hp_damage)))
}

/// The beat the guest stands in for right now, read on the host: a script move of the host does
/// not bring the guest along while one is open.
pub(crate) fn guest_stands_in_beat() -> Option<u8> {
    STAND_IN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .open_beat()
        .map(|beat| beat.beat)
}

/// Once per ghost tick on both games: the beat windows from the host's flags, the guest's move
/// to its beat's spot, held traps the host has now run, and the once-a-second line.
pub(crate) fn tick_stand_in(build: &VerifiedBuild) {
    let now_ms = net_clock_ms();
    take_host_trap_notes(now_ms);
    let fight = companion_off_fight();
    let (side, in_session) = session_side().unwrap_or((StandInSide::Host, false));
    let beat = fight.and_then(|fight| beat_from_host_flags(build, side, fight));
    let facts = StandInFacts {
        side,
        in_session,
        fight_open: fight.is_some(),
        beat,
        free_to_move: side == StandInSide::Guest && guest_free_to_move(build),
        now_ms,
    };
    let events = STAND_IN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .step(&facts);
    for event in events {
        apply_event(build, side, event);
    }
    retry_releases_the_queue_refused();
    if line_is_due() {
        log_stand_in_line(build, side);
    }
}

pub(super) fn session_side() -> Option<(StandInSide, bool)> {
    let net = net::handle()?;
    let view = net.view();
    let side = match FollowRole::from_slot(view.local_slot) {
        FollowRole::Host => StandInSide::Host,
        FollowRole::Guest => StandInSide::Guest,
        FollowRole::Unknown => return None,
    };
    let in_session = view.state == NetState::Connected && view.partner_state().is_some();
    Some((side, in_session))
}

fn take_host_trap_notes(now_ms: u64) {
    let Some(net) = net::handle() else {
        return;
    };
    for message in net.take_relayed(|message| matches!(message, Message::StandInTrapFired(_))) {
        let Message::StandInTrapFired(fired) = message else {
            continue;
        };
        log_line(&format!(
            "stand-in: the host's game ran {}, your held copy runs in a moment unless the host's copy runs here first",
            fired.name
        ));
        STAND_IN
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .host_fired(&fired.name, now_ms);
    }
}

fn tell_the_guest(build: &VerifiedBuild, name: &str, beat: &StandInBeat) {
    let Some(net) = net::handle() else {
        return;
    };
    let Some(phase) = local_phase_index(&Replicant::new(InProcessMemory::new(build))) else {
        return;
    };
    net.send_relayed(Message::StandInTrapFired(CallbackFired {
        phase,
        name: name.to_owned(),
    }));
    log_line(&format!(
        "stand-in ({}, beat {}): your game ran {name}, the guest is told so its held copy may run",
        beat.fight, beat.beat
    ));
}

fn beat_from_host_flags(
    build: &VerifiedBuild,
    side: StandInSide,
    fight: &str,
) -> Option<&'static StandInBeat> {
    let open_in = |arrays: &StoryArrays| {
        open_stand_in_beat(fight, |id| arrays.flag(StoryArray::GameFlags, id))
    };
    match side {
        StandInSide::Host => open_in(&read_live_arrays(build)?),
        StandInSide::Guest => with_host_story(open_in).flatten(),
    }
}

fn host_flags_text(build: &VerifiedBuild, side: StandInSide, beat: &StandInBeat) -> String {
    let ids: Vec<u16> = [beat.opened_by_game_flag, beat.closed_by_game_flag]
        .into_iter()
        .chain(beat.on_the_way_game_flags.iter().copied())
        .collect();
    let text = |arrays: &StoryArrays| {
        ids.iter()
            .map(|&id| {
                let on = arrays.flag(StoryArray::GameFlags, id);
                format!("g{id} {}", if on { "on" } else { "off" })
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let read = match side {
        StandInSide::Host => read_live_arrays(build).map(|arrays| text(&arrays)),
        StandInSide::Guest => with_host_story(text),
    };
    read.unwrap_or_else(|| "unreadable".to_owned())
}

fn guest_free_to_move(build: &VerifiedBuild) -> bool {
    let game = Replicant::new(InProcessMemory::new(build));
    let in_event_scene = read_world_flags(game.memory()).is_ok_and(|flags| flags.event_scene);
    // the script pause is the scripts' own (scenes, scripted moves), never a menu
    !in_event_scene && !crate::teleport::own_talk_or_script_pause_is_running(build)
}

fn apply_event(build: &VerifiedBuild, side: StandInSide, event: StandInEvent) {
    match event {
        StandInEvent::BeatOpened { beat } => log_line(&format!(
            "stand-in: beat {beat} window opened by the host's flags ({}), {}",
            beat_flags_text(build, side, beat),
            if side == StandInSide::Guest {
                "you go to its spot once your own game is past its step"
            } else {
                "guest hits on the boss are scaled down until you catch up"
            }
        )),
        StandInEvent::BeatClosed {
            beat,
            catch_up_seen,
        } => log_line(&format!(
            "stand-in: beat {beat} window closed by the host's flags ({}), {}",
            beat_flags_text(build, side, beat),
            if catch_up_seen {
                "the host caught up (its catch-up trap ran)"
            } else {
                "but no catch-up trap of the beat was seen run: the host's story moved past it another way (a story jump or a load), or the host's word is on its way"
            }
        )),
        StandInEvent::MoveGuest {
            beat,
            spot,
            yaw_degrees,
        } => move_guest(build, beat, spot, yaw_degrees),
        StandInEvent::Release(trap) => release(build, trap),
        StandInEvent::Dropped { trap, why } => log_line(&format!(
            "stand-in: your held {} (beat {}, held {} ms) is dropped: {why}",
            trap.name,
            trap.beat,
            net_clock_ms().saturating_sub(trap.held_at_ms)
        )),
    }
}

fn beat_flags_text(build: &VerifiedBuild, side: StandInSide, beat: u8) -> String {
    STAND_IN_BEATS
        .iter()
        .find(|row| row.beat == beat)
        .map_or_else(
            || "no such beat".to_owned(),
            |row| host_flags_text(build, side, row),
        )
}

fn move_guest(build: &VerifiedBuild, beat: u8, spot: [f32; 3], yaw_degrees: f32) {
    let game = Replicant::new(InProcessMemory::new(build));
    let from = game.position().ok().map(|at| [at.x, at.y, at.z]);
    let [x, y, z] = spot;
    crate::teleport::move_player_facing(build, Position { x, y, z }, yaw_degrees);
    log_line(&format!(
        "stand-in: moved you to beat {beat}'s spot ({x:.2}, {y:.2}, {z:.2}) facing {yaw_degrees:.1}, from {}",
        from.map_or_else(
            || "an unreadable position".to_owned(),
            |from| format!(
                "({:.1}, {:.1}, {:.1}), {:.1} units away",
                from[0],
                from[1],
                from[2],
                distance_between(from, spot)
            )
        )
    ));
}

fn release(build: &VerifiedBuild, trap: HeldTrap) {
    let Some(phase) = local_phase_index(&Replicant::new(InProcessMemory::new(build))) else {
        log_line(&format!(
            "stand-in: your held {} could not run, this game's map is not known; dropped",
            trap.name
        ));
        return;
    };
    log_line(&format!(
        "stand-in: your held {} (beat {}) runs now as your game's own firing, held {} ms",
        trap.name,
        trap.beat,
        net_clock_ms().saturating_sub(trap.held_at_ms)
    ));
    let call = StoryCall {
        phase,
        pack: 0,
        function: trap.name,
        args: trap.args,
    };
    if !submit_relay_call(&call, RelayPath::HeldOwnCall) {
        RELEASES_WAITING_FOR_THE_QUEUE
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(call);
    }
}

fn retry_releases_the_queue_refused() {
    let waiting = std::mem::take(
        &mut *RELEASES_WAITING_FOR_THE_QUEUE
            .lock()
            .unwrap_or_else(PoisonError::into_inner),
    );
    for call in waiting {
        if !submit_relay_call(&call, RelayPath::HeldOwnCall) {
            RELEASES_WAITING_FOR_THE_QUEUE
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(call);
        }
    }
}

fn line_is_due() -> bool {
    let mut last = LAST_LINE.lock().unwrap_or_else(PoisonError::into_inner);
    let now = Instant::now();
    if last.is_some_and(|at| now.saturating_duration_since(at) < STAND_IN_LINE_INTERVAL) {
        return false;
    }
    *last = Some(now);
    true
}

fn log_stand_in_line(build: &VerifiedBuild, side: StandInSide) {
    let Some(report) = STAND_IN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take_report()
    else {
        return;
    };
    let beat = report.beat;
    let guest_at = match side {
        StandInSide::Guest => Replicant::new(InProcessMemory::new(build))
            .position()
            .ok()
            .map(|at| [at.x, at.y, at.z]),
        StandInSide::Host => net::handle().and_then(|net| {
            net.view()
                .partner_state()
                .map(|remote| remote.state.position)
        }),
    };
    let distance = guest_at.map_or_else(
        || "unknown".to_owned(),
        |at| format!("{:.1}", distance_between(at, beat.spot)),
    );
    let held = if report.held.is_empty() {
        "none".to_owned()
    } else {
        report.held.join(", ")
    };
    log_line(&format!(
        "stand-in ({}, beat {}, {}): the guest is {distance} units from the beat's spot, held trap firings now {held} ({} held since the last line), traps the host ran and told {}, guest hits on boss {}:{} scaled x{GUEST_HIT_SCALE_IN_A_STAND_IN_BEAT:.2} {}, host flags {}",
        beat.fight,
        beat.beat,
        match side {
            StandInSide::Guest => "guest",
            StandInSide::Host => "host",
        },
        report.holds,
        report.traps_told,
        beat.boss.group,
        beat.boss.ordinal,
        report.hits_scaled,
        host_flags_text(build, side, beat)
    ));
}
