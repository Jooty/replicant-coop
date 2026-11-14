use std::sync::{LazyLock, Mutex, PoisonError};

use replicant_game::blocks::distance_between;
use replicant_game::bombs::{
    BreakableState, BrokenWalls, WALL_MATCH_DISTANCE, breakable_class, nearest_breakable,
    read_breakables,
};
use replicant_game::effects::read_actor_position;
use replicant_proto::{Message, WallBroken};

use crate::bombs::wall_hook::break_wall_here;
use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::door_sync::actor_of_layout_id;
use crate::memory::InProcessMemory;
use crate::net::{self, net_clock_ms};

static WALLS: LazyLock<Mutex<BrokenWalls>> = LazyLock::new(Mutex::default);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CopyOutcome {
    Broke(u64),
    AlreadyBreaking(u64),
    Refused(u64),
    NotFound,
}

pub fn tick_walls(build: &VerifiedBuild) {
    let Some(net) = net::handle() else {
        return;
    };
    let arrived = net.take_relayed(|message| matches!(message, Message::WallBroken(_)));
    let local_phase = net.view().local.as_ref().map(|local| local.phase);
    let now_ms = net_clock_ms();
    let mut walls = WALLS.lock().unwrap_or_else(PoisonError::into_inner);
    for message in arrived {
        let Message::WallBroken(wall) = message else {
            continue;
        };
        let kept = walls.note(wall);
        log_line(&format!(
            "walls: the partner broke wall {:?} at ({:.1}, {:.1}, {:.1}) in zone {} (you are in {local_phase:?}){}",
            wall.layout_id,
            wall.position[0],
            wall.position[1],
            wall.position[2],
            wall.phase,
            if kept { "" } else { ", list full, dropped" }
        ));
    }
    for wall in walls.due(local_phase, now_ms) {
        match copy_break(build, wall) {
            CopyOutcome::Broke(actor) | CopyOutcome::AlreadyBreaking(actor) => {
                walls.copied(wall);
                log_line(&format!(
                    "walls: wall {:?} ({actor:#x}) is broken here too",
                    wall.layout_id
                ));
            }
            CopyOutcome::Refused(actor) => log_line(&format!(
                "walls: wall {:?} ({actor:#x}) did not take the copied blast, trying again",
                wall.layout_id
            )),
            CopyOutcome::NotFound => {}
        }
    }
    for wall in walls.take_given_up(now_ms) {
        log_line(&format!(
            "walls: gave up on the partner's wall {:?}: no breakable within {WALL_MATCH_DISTANCE} of ({:.1}, {:.1}, {:.1}) here",
            wall.layout_id, wall.position[0], wall.position[1], wall.position[2]
        ));
    }
}

fn copy_break(build: &VerifiedBuild, wall: WallBroken) -> CopyOutcome {
    let memory = InProcessMemory::new(build);
    let Some(actor) = find_wall(build, &memory, wall) else {
        return CopyOutcome::NotFound;
    };
    match BreakableState::read(&memory, actor) {
        Ok(state) if !state.is_whole() => CopyOutcome::AlreadyBreaking(actor),
        Ok(_) if break_wall_here(build, actor) => CopyOutcome::Broke(actor),
        _ => CopyOutcome::Refused(actor),
    }
}

fn find_wall(build: &VerifiedBuild, memory: &InProcessMemory, wall: WallBroken) -> Option<u64> {
    let by_id = wall
        .layout_id
        .and_then(|id| actor_of_layout_id(build, id))
        .map(|actor| actor as u64)
        .filter(|actor| breakable_class(memory, *actor).is_some())
        .filter(|actor| {
            read_actor_position(memory, *actor).is_ok_and(|position| {
                distance_between(position, wall.position) <= WALL_MATCH_DISTANCE
            })
        });
    by_id.or_else(|| {
        let breakables = read_breakables(memory).ok()?;
        nearest_breakable(&breakables, wall.position, WALL_MATCH_DISTANCE)
            .map(|breakable| breakable.actor)
    })
}
