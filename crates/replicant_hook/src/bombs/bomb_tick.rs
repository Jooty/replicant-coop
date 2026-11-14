use core::ffi::c_void;
use std::sync::PoisonError;
use std::sync::atomic::{AtomicU64, Ordering};

use replicant_game::bombs::{
    BOMB_BLAST_EFFECT_ID, BOMB_BLAST_SOUND_ID, BOMB_BLAST_SOUND_VOLUME, BOMB_SCAN_INTERVAL_MS,
    read_bombs,
};
use replicant_game::offsets::{AUDIO_MAN_RVA, SOUND_BY_ID_WRAPPER_FUNCTION_RVA};
use replicant_game::{GameMemory, Replicant};
use replicant_proto::{
    BULLET_TRANSFORM_FLOATS, BombBlast, Message, PowerModelEnded, PowerModelKind, PowerModelState,
};

use crate::bombs::blast_hook::OWN_BOMBS;
use crate::build_gate::VerifiedBuild;
use crate::bullets::create_effect;
use crate::diagnostics::log_line;
use crate::memory::InProcessMemory;
use crate::net::{self, NetSharedExt, net_clock_ms};

static LAST_SCAN_MS: AtomicU64 = AtomicU64::new(0);

type SoundByIdWrapper = unsafe extern "system" fn(*mut c_void, u32, *const f32, f32, f32) -> u8;

#[repr(align(16))]
struct AlignedPoint([f32; 4]);

pub fn tick_bombs(build: &VerifiedBuild) {
    let Some(net) = net::handle() else {
        return;
    };
    let view = net.view();
    let local_phase = view.local.as_ref().map(|local| local.phase);
    for message in net.take_relayed(|message| matches!(message, Message::BombBlast(_))) {
        if let Message::BombBlast(blast) = message {
            show_partner_blast(build, blast, local_phase);
        }
    }
    let now_ms = net_clock_ms();
    if !view.partner_in_play() {
        OWN_BOMBS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take_all(now_ms);
        return;
    }
    if now_ms.saturating_sub(LAST_SCAN_MS.load(Ordering::Relaxed)) < BOMB_SCAN_INTERVAL_MS {
        return;
    }
    LAST_SCAN_MS.store(now_ms, Ordering::Relaxed);
    let game = Replicant::new(InProcessMemory::new(build));
    let Ok(player) = game.playable_actor() else {
        return;
    };
    let Ok(bombs) = read_bombs(game.memory()) else {
        return;
    };
    let changes = OWN_BOMBS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .update(&bombs, player, now_ms);
    for bomb in &changes.taken {
        log_line(&format!(
            "bombs: you took bomb #{} ({:#x}) at ({:.1}, {:.1}, {:.1}); the partner is shown it",
            bomb.slot, bomb.actor, bomb.pose.position.x, bomb.pose.position.y, bomb.pose.position.z
        ));
    }
    for (slot, lasted_ms) in changes.gone {
        log_line(&format!(
            "bombs: your bomb #{slot} is gone after {lasted_ms} ms"
        ));
        net.send_relayed(Message::PowerEnded(PowerModelEnded {
            instance: slot,
            lasted_ms: u32::try_from(lasted_ms).unwrap_or(u32::MAX),
        }));
    }
    for bomb in changes.live {
        let state = PowerModelState {
            kind: PowerModelKind::Bomb010,
            instance: bomb.slot,
            position: [
                bomb.pose.position.x,
                bomb.pose.position.y,
                bomb.pose.position.z,
            ],
            yaw_radians: bomb.pose.yaw,
        };
        if state.validate().is_ok() {
            net.send_relayed(Message::PowerModel(state));
        }
    }
}

fn show_partner_blast(build: &VerifiedBuild, blast: BombBlast, local_phase: Option<u16>) {
    let [x, y, z] = blast.position;
    if Some(blast.phase) != local_phase {
        log_line(&format!(
            "bombs: the partner's bomb went off in zone {}, you are in {local_phase:?}; not shown",
            blast.phase
        ));
        return;
    }
    let transform: [f32; BULLET_TRANSFORM_FLOATS] =
        [1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, y, 0.0, 0.0, 1.0, z];
    let effect = create_effect(build, BOMB_BLAST_EFFECT_ID, &transform).is_some();
    let sound = play_blast_sound(build, blast.position);
    log_line(&format!(
        "bombs: the partner's bomb went off at ({x:.1}, {y:.1}, {z:.1}): effect shown {effect}, sound played {sound}"
    ));
}

fn play_blast_sound(build: &VerifiedBuild, position: [f32; 3]) -> bool {
    let base = build.module_base();
    if InProcessMemory::new(build)
        .read_u64(base + u64::from(AUDIO_MAN_RVA))
        .is_ok_and(|first_word| first_word == 0)
    {
        return false;
    }
    let point = AlignedPoint([position[0], position[1], position[2], 1.0]);
    let function = (base + u64::from(SOUND_BY_ID_WRAPPER_FUNCTION_RVA)) as usize;
    #[expect(unsafe_code, reason = "calls the game's play-sound-by-id wrapper")]
    // SAFETY: the RVA starts SoundMap::PlayByIdWrapper (audio manager, sound id, position, volume,
    // float -> bool) in the verified build, called as the bomb's own blast calls it with the
    // audio manager singleton, which was just checked to be built; this runs on the game thread.
    let played = unsafe {
        core::mem::transmute::<usize, SoundByIdWrapper>(function)(
            (base + u64::from(AUDIO_MAN_RVA)) as *mut c_void,
            BOMB_BLAST_SOUND_ID,
            point.0.as_ptr(),
            BOMB_BLAST_SOUND_VOLUME,
            0.0,
        )
    };
    played != 0
}
