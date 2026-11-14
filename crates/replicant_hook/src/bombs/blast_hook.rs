use core::ffi::c_void;
use core::ptr;
use std::panic;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock, PoisonError};

use replicant_game::GameMemory;
use replicant_game::blocks::distance_between;
use replicant_game::bombs::{
    ATTACK_HIT_SERIAL_RVA, BLAST_NEIGHBOUR_LOG_DISTANCE, BOMB_EXPLODE_FUNCTION_RVA, BreakableState,
    MAX_BLAST_NEIGHBOURS_LOGGED, OwnBombHits, OwnBombs, breakable_class, pool_actors,
};
use replicant_game::effects::read_actor_position;
use replicant_proto::{BombBlast, Message};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::game_hooks::hook_address;
use crate::memory::InProcessMemory;
use crate::net::{self, net_clock_ms};

pub(super) static OWN_BOMBS: LazyLock<Mutex<OwnBombs>> = LazyLock::new(Mutex::default);
static OWN_BOMB_HITS: LazyLock<Mutex<OwnBombHits>> = LazyLock::new(Mutex::default);
static HOOKED_BUILD: OnceLock<VerifiedBuild> = OnceLock::new();
static ORIGINAL_EXPLODE: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

type Explode = unsafe extern "system" fn(*mut c_void);

pub(super) fn install(build: VerifiedBuild) {
    if HOOKED_BUILD.set(build).is_err() {
        return;
    }
    #[expect(unsafe_code, reason = "hooks the bomb's explode function by address")]
    // SAFETY: the RVA starts CActorBomb's explode (bomb -> nothing) in the verified build, called
    // by its update when the fuse runs out and by its hit method; the replacement matches it.
    unsafe {
        hook_address(
            "CActorBomb::Explode",
            (build.module_base() + u64::from(BOMB_EXPLODE_FUNCTION_RVA)) as usize,
            explode_detour as *const () as *mut c_void,
            &ORIGINAL_EXPLODE,
        );
    }
}

/// Whether an attack with this hit id came from a blast of one of this player's bombs.
pub(crate) fn is_own_bomb_hit(hit_id: u32) -> bool {
    OWN_BOMB_HITS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_own(hit_id, net_clock_ms())
}

#[expect(
    unsafe_code,
    reason = "calls the original explode function through the trampoline MinHook built"
)]
extern "system" fn explode_detour(bomb: *mut c_void) {
    let _ = panic::catch_unwind(|| note_blast(bomb as u64));
    let original = ORIGINAL_EXPLODE.load(Ordering::Acquire);
    if original.is_null() {
        return;
    }
    // SAFETY: the trampoline continues into the original function, which has this signature.
    unsafe { core::mem::transmute::<*mut c_void, Explode>(original)(bomb) };
}

fn note_blast(bomb: u64) {
    let Some(build) = HOOKED_BUILD.get() else {
        return;
    };
    let memory = InProcessMemory::new(build);
    let Ok(position) = read_actor_position(&memory, bomb) else {
        return;
    };
    // The blast's attack takes the serial as its hit id, then the game counts the serial up.
    let hit_id = memory
        .read_i32(build.module_base() + u64::from(ATTACK_HIT_SERIAL_RVA))
        .map_or(0, i32::cast_unsigned);
    let own = OWN_BOMBS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_own(bomb);
    log_line(&format!(
        "bombs: {} bomb {bomb:#x} went off at ({:.1}, {:.1}, {:.1}), hit id {hit_id}",
        if own { "your" } else { "a never-held" },
        position[0],
        position[1],
        position[2]
    ));
    log_neighbours(&memory, position);
    if !own {
        return;
    }
    OWN_BOMB_HITS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .note(hit_id, net_clock_ms());
    let Some(net) = net::handle() else {
        return;
    };
    let Some(phase) = net.view().local.as_ref().map(|local| local.phase) else {
        return;
    };
    let blast = BombBlast { phase, position };
    if blast.validate().is_ok() {
        net.send_relayed(Message::BombBlast(blast));
    }
}

fn log_neighbours(memory: &InProcessMemory, blast: [f32; 3]) {
    let Ok(actors) = pool_actors(memory) else {
        return;
    };
    let base = memory.module_base();
    let mut near: Vec<(f32, u64)> = actors
        .into_iter()
        .filter_map(|(_, actor)| {
            let position = read_actor_position(memory, actor).ok()?;
            let distance = distance_between(position, blast);
            (distance <= BLAST_NEIGHBOUR_LOG_DISTANCE).then_some((distance, actor))
        })
        .collect();
    near.sort_by(|(left, _), (right, _)| left.total_cmp(right));
    let lines: Vec<String> = near
        .into_iter()
        .take(MAX_BLAST_NEIGHBOURS_LOGGED)
        .map(|(distance, actor)| {
            let class = memory
                .read_u64(actor)
                .map_or(0, |vtable| vtable.wrapping_sub(base));
            let breakable = breakable_class(memory, actor)
                .and_then(|kind| {
                    BreakableState::read(memory, actor).ok().map(|state| {
                        format!(
                            " {kind:?} timer {:.2} hits {} mode {}",
                            state.break_timer, state.hits_left, state.mode
                        )
                    })
                })
                .unwrap_or_default();
            format!("{actor:#x} class rva {class:#x} at {distance:.1}{breakable}")
        })
        .collect();
    log_line(&format!(
        "bombs: actors near the blast: {}",
        if lines.is_empty() {
            "none".to_owned()
        } else {
            lines.join("; ")
        }
    ));
}
