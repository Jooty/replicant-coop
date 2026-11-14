use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, PoisonError};

use replicant_game::enemy_bullets::{
    CueAdmission, ENEMY_BULLET_SCAN_INTERVAL_MS, EnemyBulletCensus, EnemyBulletKinds,
    EnemyBulletSender, FlightRefusals, LaserBeamTally, REPEAT_WITHIN_THE_GAP,
    SHOT_CUE_SUMMARY_INTERVAL_MS, ShotCueGate, ShotCueTally, ShotInstances, SpriteBulletKinds,
    UncopiedTally, look_scale, scan_enemy_bullets,
};
use replicant_proto::{EnemyId, Message, ShotCue};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::enemies::enemy_id_text;
use crate::enemy_bullets::ENEMY_BULLET_SUMMARY_INTERVAL_MS;
use crate::enemy_bullets::script_commands::set_shared_enemy_actors;
use crate::enemy_bullets::shot_cues::take_captured;
use crate::memory::InProcessMemory;
use crate::net::{NetThread, net_clock_ms};

static SENDING: LazyLock<Mutex<Sending>> = LazyLock::new(Mutex::default);

#[derive(Debug, Default)]
struct Sending {
    sender: EnemyBulletSender,
    kinds: EnemyBulletKinds,
    sprite_kinds: SpriteBulletKinds,
    last_scan_ms: u64,
    messages: u32,
    sprite_messages: u32,
    ended: u32,
    most_in_the_air: EnemyBulletCensus,
    uncopied: UncopiedTally,
    beams: LaserBeamTally,
    refusals: FlightRefusals,
    last_summary_ms: u64,
    instances: ShotInstances,
    cue_gate: ShotCueGate,
    cue_tally: ShotCueTally,
    last_cue_summary_ms: u64,
}

pub(super) fn send_enemy_bullets(
    build: &VerifiedBuild,
    net: &NetThread,
    enemies: Option<&HashMap<u64, EnemyId>>,
) {
    let mut sending = SENDING.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(enemies) = enemies else {
        // Not hosting a partner in play: the guest drops what it shows when the flights stop.
        let _ = sending.sender.end_all();
        set_shared_enemy_actors(core::iter::empty());
        let _ = take_captured();
        return;
    };
    set_shared_enemy_actors(enemies.keys().copied());
    let now_ms = net_clock_ms();
    if now_ms.saturating_sub(sending.last_scan_ms) < ENEMY_BULLET_SCAN_INTERVAL_MS {
        return;
    }
    sending.last_scan_ms = now_ms;
    let memory = InProcessMemory::new(build);
    let Ok(scan) = scan_enemy_bullets(&memory, enemies) else {
        return;
    };
    sending.most_in_the_air.keep_highest(&scan.census);
    for line in sending.uncopied.add_scan(&scan.uncopied) {
        log_line(&line);
    }
    for line in sending.beams.add_scan(&scan.bullets) {
        log_line(&line);
    }
    sending.instances.note_scan(&scan.bullets);
    for flight in sending.sender.update(&scan.bullets, now_ms) {
        if let Err(error) = flight.validate() {
            sending.refusals.note(error.to_string());
            continue;
        }
        if let Some(look) = flight.flight.look
            && sending.kinds.first_sighting(look.effect_ids)
        {
            log_line(&format!(
                "enemy bullets: kind {:?} from enemy {} sent",
                look.effect_ids,
                enemy_id_text(flight.enemy)
            ));
        }
        if let Some(sprite) = flight.sprite
            && sending.sprite_kinds.first_sighting(&sprite)
        {
            log_line(&format!(
                "enemy bullets: sprite kind {} of size {} (alpha {}, angle {}) from enemy {} sent",
                sprite.kind,
                sprite.size,
                sprite.alpha,
                sprite.angle,
                enemy_id_text(flight.enemy)
            ));
        }
        if flight.scale_growth_per_second > 0.0
            && let Some(look) = flight.flight.look
            && sending
                .sender
                .started_in_last_update()
                .contains(&flight.flight.instance)
        {
            log_line(&format!(
                "wide shot: {} from enemy {} sent first at scale {:.1}, growing {:.1} times its size a second",
                flight.flight.instance,
                enemy_id_text(flight.enemy),
                look_scale(&look),
                flight.scale_growth_per_second
            ));
        }
        sending.messages += 1;
        sending.beams.note_sent(flight.flight.instance);
        if flight.sprite.is_some() {
            sending.sprite_messages += 1;
        }
        if flight.flight.ended {
            sending.ended += 1;
        }
        net.send_relayed(Message::EnemyBulletFlight(flight));
    }
    let resized = sending.sender.resized_in_last_update().to_vec();
    for (instance, scale) in resized {
        sending.cue_tally.note_resize(instance, scale);
    }
    // After the flights: a trail made as a shot starts finds its copy, an impact its end.
    send_shot_cues(&mut sending, net, now_ms);
    if now_ms.saturating_sub(sending.last_summary_ms) >= ENEMY_BULLET_SUMMARY_INTERVAL_MS {
        sending.last_summary_ms = now_ms;
        if sending.messages > 0 {
            log_line(&format!(
                "enemy bullets: last 5 s sent {} flight messages ({} of them for sprites), {} bullets ended, {} in the air",
                sending.messages,
                sending.sprite_messages,
                sending.ended,
                sending.sender.len()
            ));
            sending.messages = 0;
            sending.sprite_messages = 0;
            sending.ended = 0;
        }
        let most = core::mem::take(&mut sending.most_in_the_air);
        if !most.is_empty() {
            log_line(&format!(
                "enemy bullets: last 5 s most in the air at once: {} normal ({} more not drawn), {} script with an effect, {} script sprite ({} more not drawn yet), {} script with no effect to copy, {} trace, {} past the limit",
                most.normal,
                most.normal_not_drawn,
                most.script_with_effect,
                most.script_sprite,
                most.script_sprite_not_drawn,
                most.script_without_effect,
                most.trace,
                most.past_the_limit
            ));
        }
        if let Some(line) = sending.uncopied.take_summary() {
            log_line(&line);
        }
        if let Some(line) = sending.beams.take_summary() {
            log_line(&line);
        }
        if let Some(line) = sending.refusals.take_summary() {
            log_line(&line);
        }
    }
}

fn send_shot_cues(sending: &mut Sending, net: &NetThread, now_ms: u64) {
    let (captured, overflowed) = take_captured();
    for _ in 0..overflowed {
        sending.cue_tally.note_dropped("over the capture queue");
    }
    for captured in captured {
        let admission = sending
            .cue_gate
            .admit(captured.bullet, captured.kind, captured.id, now_ms);
        if admission == CueAdmission::RepeatTooSoon {
            sending.cue_tally.note_dropped(REPEAT_WITHIN_THE_GAP);
            continue;
        }
        let instance = sending.instances.instance_of(captured.bullet);
        let cue = ShotCue {
            instance,
            kind: captured.kind,
            id: captured.id,
            transform: captured.transform,
            offset: captured.offset,
            style: captured.style,
            sound_levels: captured.sound_levels,
        };
        if cue.validate().is_err() {
            sending.cue_tally.note_dropped("not valid on the wire");
            continue;
        }
        sending.cue_tally.note_cue(instance, cue.kind);
        net.send_relayed(Message::ShotCue(cue));
    }
    if now_ms.saturating_sub(sending.last_cue_summary_ms) >= SHOT_CUE_SUMMARY_INTERVAL_MS {
        sending.last_cue_summary_ms = now_ms;
        if let Some(line) = sending.cue_tally.take_line("sent", sending.sender.len()) {
            log_line(&line);
        }
    }
}
