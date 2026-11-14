use std::panic;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use replicant_game::{Displacement, Position, Replicant};

use crate::build_gate::VerifiedBuild;
use crate::game_hooks::noclip;
use crate::memory::InProcessMemory;

static PENDING: Mutex<Option<Position>> = Mutex::new(None);
static HAS_PENDING: AtomicBool = AtomicBool::new(false);

pub fn teleport_available() -> bool {
    noclip::is_installed()
}

pub fn request_teleport(target: Position) {
    *PENDING.lock().unwrap_or_else(PoisonError::into_inner) = Some(target);
    HAS_PENDING.store(true, Ordering::Release);
}

pub(super) fn apply_pending(build: &VerifiedBuild, actor: usize) -> bool {
    if !HAS_PENDING.load(Ordering::Acquire) {
        return false;
    }
    panic::catch_unwind(|| {
        let mut game = Replicant::new(InProcessMemory::new(build));
        if game
            .playable_actor()
            .is_ok_and(|player| player != actor as u64)
        {
            return false;
        }
        let Some(target) = PENDING
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        else {
            return false;
        };
        HAS_PENDING.store(false, Ordering::Release);
        let Ok(here) = game.position() else {
            return false;
        };
        let step = Displacement {
            x: target.x - here.x,
            y: target.y - here.y,
            z: target.z - here.z,
        };
        crate::teleport::note_mod_move();
        game.write_noclip_step(actor as u64, step).is_ok()
    })
    .unwrap_or(false)
}
