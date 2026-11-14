use std::sync::{Mutex, PoisonError};

use replicant_game::blocks::distance_between;
use replicant_game::enemies::unique_companion_pose;
use replicant_game::kaine::{
    GuestSceneStandIn, ScenePlaceSource, SceneStandIn, SceneStandInEvent, SceneStandInFacts,
    StandInSide, scene_stand_in_place,
};
use replicant_game::marker::read_world_flags;
use replicant_game::{Position, Replicant};
use replicant_proto::{DialogueUpdate, StoryArray};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::kaine::companion_rows_from_scripts;
use crate::kaine::stand_in::session_side;
use crate::memory::InProcessMemory;
use crate::net::net_clock_ms;
use crate::story_progress::with_host_story;

static RULE: Mutex<GuestSceneStandIn> = Mutex::new(GuestSceneStandIn::new());

/// A line or the end of the partner's talk reached this game: a listed scene's talk opens it.
pub(crate) fn scene_stand_in_partner_talk(update: DialogueUpdate) {
    let mut rule = RULE.lock().unwrap_or_else(PoisonError::into_inner);
    rule.use_rows_from_scripts(companion_rows_from_scripts());
    match update {
        DialogueUpdate::Line(line) => rule.partner_talk_line(line.talk_id, net_clock_ms()),
        DialogueUpdate::Ended => rule.partner_talk_ended(),
    }
}

/// A script function ran on this game (a pop that was let run, or a trace).
pub(crate) fn scene_stand_in_function_ran(name: &str) {
    RULE.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .function_ran(name);
}

/// Once per ghost tick: places the guest when a listed scene opens and lets it go when it ends.
pub(crate) fn tick_scene_stand_in(build: &VerifiedBuild) {
    let (side, in_session) = session_side().unwrap_or((StandInSide::Host, false));
    let game = Replicant::new(InProcessMemory::new(build));
    let pack = game.current_phase().ok();
    let open = RULE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .open_scene();
    let closing_flag_on = open.is_some_and(|row| {
        side == StandInSide::Guest
            && with_host_story(|arrays| arrays.flag(StoryArray::GameFlags, row.closed_by_game_flag))
                .unwrap_or(false)
    });
    let facts = SceneStandInFacts {
        side,
        in_session,
        pack: pack.as_deref(),
        closing_flag_on,
        free_to_move: open.is_some() && side == StandInSide::Guest && free_to_move(build),
        now_ms: net_clock_ms(),
    };
    let events = {
        let mut rule = RULE.lock().unwrap_or_else(PoisonError::into_inner);
        rule.use_rows_from_scripts(companion_rows_from_scripts());
        rule.step(&facts)
    };
    for event in events {
        apply_event(build, event);
    }
}

fn free_to_move(build: &VerifiedBuild) -> bool {
    let game = Replicant::new(InProcessMemory::new(build));
    let in_event_scene = read_world_flags(game.memory()).is_ok_and(|flags| flags.event_scene);
    // a window that only mirrors the host's talk does not count, and a menu never holds this
    !in_event_scene
        && !crate::dialogue::own_talk_window_open()
        && !crate::game_hooks::script_pause_in_force()
}

fn apply_event(build: &VerifiedBuild, event: SceneStandInEvent) {
    match event {
        SceneStandInEvent::Opened { row } => log_line(&format!(
            "scene stand-in ({}): the host's talk {} opened, you are put at the companion's place once your game is free (pack {}); she stays on for {}",
            row.scene,
            row.talk_id,
            row.pack,
            row.companion_kept_for.join(", ")
        )),
        SceneStandInEvent::Place { row } => place_guest(build, row),
        SceneStandInEvent::Released { row, placed, why } => log_line(&format!(
            "scene stand-in ({}): released, you are free again ({why}; {})",
            row.scene,
            if placed {
                "you had been placed"
            } else {
                "you were never placed"
            }
        )),
    }
}

fn place_guest(build: &VerifiedBuild, row: &SceneStandIn) {
    let game = Replicant::new(InProcessMemory::new(build));
    let from = game.position().ok().map(|at| [at.x, at.y, at.z]);
    let her = unique_companion_pose(game.memory());
    let place = scene_stand_in_place(row, her);
    let [x, y, z] = place.spot;
    crate::teleport::move_player_facing(build, Position { x, y, z }, place.heading_degrees);
    log_line(&format!(
        "scene stand-in ({}): placed you at the companion's place ({x:.2}, {y:.2}, {z:.2}) facing {:.1}, read from {}, from {}",
        row.scene,
        place.heading_degrees,
        match (place.source, her) {
            (ScenePlaceSource::HerActor, _) => "her actor".to_owned(),
            (ScenePlaceSource::Script, Some(pose)) => format!(
                "the script's numbers (her actor was at ({:.1}, {:.1}, {:.1}) yaw {:.2}, not at the script's spot)",
                pose.position.x, pose.position.y, pose.position.z, pose.yaw
            ),
            (ScenePlaceSource::Script, None) => {
                "the script's numbers (her actor is not live here)".to_owned()
            }
        },
        from.map_or_else(
            || "an unreadable position".to_owned(),
            |from| format!(
                "({:.1}, {:.1}, {:.1}), {:.1} units away",
                from[0],
                from[1],
                from[2],
                distance_between(from, place.spot)
            )
        )
    ));
}
