use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, PoisonError};

use replicant_game::Replicant;
use replicant_game::bullet_flights::{BULLET_SCAN_INTERVAL_MS, BulletTracker, bullets_of_player};
use replicant_proto::{BULLET_EFFECT_SLOTS, Message};

use crate::build_gate::VerifiedBuild;
use crate::bullets::SUMMARY_INTERVAL_MS;
use crate::diagnostics::log_line;
use crate::kaine::sent_shot_ids;
use crate::memory::InProcessMemory;
use crate::net::NetSharedExt;
use crate::net::{self, net_clock_ms};

static LAST_SCAN_MS: AtomicU64 = AtomicU64::new(0);
static SENDER: LazyLock<Mutex<Sender>> = LazyLock::new(Mutex::default);

#[derive(Debug, Default)]
struct Sender {
    tracker: BulletTracker,
    kinds: Vec<[i32; BULLET_EFFECT_SLOTS]>,
    messages: u32,
    shots_ended: u32,
    last_summary_ms: u64,
}

pub fn tick_own_bullets(build: &VerifiedBuild) {
    let Some(net) = net::handle() else {
        return;
    };
    let mut sender = SENDER.lock().unwrap_or_else(PoisonError::into_inner);
    if !net.view().partner_in_play() {
        sender.tracker.end_all();
        return;
    }
    let now_ms = net_clock_ms();
    if now_ms.saturating_sub(LAST_SCAN_MS.load(Ordering::Relaxed)) < BULLET_SCAN_INTERVAL_MS {
        return;
    }
    LAST_SCAN_MS.store(now_ms, Ordering::Relaxed);
    let game = Replicant::new(InProcessMemory::new(build));
    let Ok(player) = game.playable_actor() else {
        return;
    };
    let Ok(bullets) = bullets_of_player(game.memory(), player) else {
        return;
    };
    for mut flight in sender.tracker.update(&bullets, now_ms) {
        if flight.validate().is_err() {
            continue;
        }
        if let Some(look) = flight.look.as_mut() {
            look.effect_ids = sent_shot_ids(look.effect_ids);
        }
        if let Some(look) = flight.look
            && !sender.kinds.contains(&look.effect_ids)
        {
            sender.kinds.push(look.effect_ids);
            log_line(&format!(
                "bullets: sending your shot of kind {:?} ({} kinds seen)",
                look.effect_ids,
                sender.kinds.len()
            ));
        }
        sender.messages += 1;
        if flight.ended {
            sender.shots_ended += 1;
        }
        net.send_relayed(Message::BulletFlight(flight));
    }
    if now_ms.saturating_sub(sender.last_summary_ms) >= SUMMARY_INTERVAL_MS {
        sender.last_summary_ms = now_ms;
        if sender.messages > 0 {
            log_line(&format!(
                "bullets: last 5 s sent {} flight messages, {} shots ended, {} in the air",
                sender.messages,
                sender.shots_ended,
                sender.tracker.len()
            ));
            sender.messages = 0;
            sender.shots_ended = 0;
        }
    }
}
