use std::collections::VecDeque;
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::Instant;

use replicant_game::effects::MAX_PUPPET_EFFECTS_PER_TICK;
use replicant_game::effects::{
    HitSparkNames, LiveEffects, LookAttempt, PUPPET_EFFECT_MAX_LIFETIME_MS, finish_effect,
    look_attempt, look_log_line, look_needs_write, read_effect_look, sound_gain, write_effect_look,
};
use replicant_game::puppet::actor_is_live;
use replicant_proto::{EffectLook, EffectPlay, Message};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::effects::calls::{
    PlayedEffect, align_anchor_offset, distance_to_listener, move_world_effect, play_effect,
    play_sound, spawn_world_effect,
};
use crate::effects::hooks::{note_partner_hit_effect_played, send_ended_effects, set_sending};
use crate::effects::report::{
    DROPPED_WITHOUT_PUPPET, PLAY_FAILED, PLAYED_EFFECTS, PLAYED_SOUNDS, PLAYED_STOPS,
    SILENCED_BY_DISTANCE, count, log_limited, log_summary, note_world_effect_shown,
};
use crate::effects::voice::{
    forget_sent_character, play_as_partner_character, remember_partner_character,
    send_local_character,
};
use crate::effects::{
    MAX_PENDING_EVENTS, PLAYBACK_DELAY, RETAINED_BLOCK_EXTRA_MS, SUMMARY_INTERVAL_MS,
};
use crate::memory::InProcessMemory;
use crate::net::NetSharedExt;
use crate::net::{self, net_clock_ms};
use crate::puppet::{book_actor_address, book_actor_moved_to, first_actor_address};

static MIRROR: LazyLock<Mutex<Mirror>> = LazyLock::new(|| Mutex::new(Mirror::new()));

struct WaitingLook {
    handle: u32,
    name: String,
    look: EffectLook,
    frames_waited: u32,
}

struct Mirror {
    waiting_looks: Vec<WaitingLook>,
    logged_looks: HitSparkNames,
    pending: VecDeque<(Instant, Message)>,
    live: LiveEffects,
    retained_blocks: VecDeque<(u64, Vec<u64>)>,
    last_summary_ms: u64,
}

impl Mirror {
    fn new() -> Self {
        Self {
            waiting_looks: Vec::new(),
            logged_looks: HitSparkNames::default(),
            pending: VecDeque::new(),
            live: LiveEffects::new(),
            retained_blocks: VecDeque::new(),
            last_summary_ms: 0,
        }
    }

    fn finish_effects(&mut self, build: &VerifiedBuild, handles: &[u32]) {
        let mut memory = InProcessMemory::new(build);
        for handle in handles {
            let _ = finish_effect(&mut memory, *handle);
        }
    }

    fn play_aligned_effect(
        &mut self,
        build: &VerifiedBuild,
        puppet: u64,
        effect: &EffectPlay,
    ) -> Result<PlayedEffect, String> {
        align_anchor_offset(build, puppet);
        play_effect(build, puppet, effect)
    }

    fn apply_waiting_looks(&mut self, build: &VerifiedBuild) {
        let mut memory = InProcessMemory::new(build);
        for mut waiting in core::mem::take(&mut self.waiting_looks) {
            let own = read_effect_look(&memory, waiting.handle).ok().flatten();
            match look_attempt(waiting.frames_waited, own.is_some()) {
                LookAttempt::Wait => {
                    waiting.frames_waited += 1;
                    self.waiting_looks.push(waiting);
                }
                LookAttempt::GiveUp => log_line(&format!(
                    "effects: effect look {} not applied, the puppet effect {:#x} has no instance",
                    waiting.name, waiting.handle
                )),
                LookAttempt::Use => {
                    let Some(own) = own else {
                        continue;
                    };
                    if self.logged_looks.is_first(&waiting.name) {
                        log_line(&look_log_line(&waiting.name, &waiting.look, &own));
                    }
                    if look_needs_write(&waiting.look, &own) {
                        let _ = write_effect_look(&mut memory, waiting.handle, &waiting.look);
                    }
                }
            }
        }
    }

    fn expire(&mut self, build: &VerifiedBuild, now_ms: u64) {
        let expired = self.live.take_expired(now_ms);
        let expired_handles: Vec<u32> = expired.iter().map(|(handle, _)| *handle).collect();
        self.finish_effects(build, &expired_handles);
        for (handle, sender) in &expired {
            log_line(&format!(
                "effects: puppet effect {handle:#x} (sender handle {sender:#x}) ended by the {PUPPET_EFFECT_MAX_LIFETIME_MS} ms limit, no stop arrived"
            ));
        }
        let block_life_ms = PUPPET_EFFECT_MAX_LIFETIME_MS + RETAINED_BLOCK_EXTRA_MS;
        while self
            .retained_blocks
            .front()
            .is_some_and(|(started, _)| now_ms.saturating_sub(*started) >= block_life_ms)
        {
            self.retained_blocks.pop_front();
        }
    }

    fn play(&mut self, build: &VerifiedBuild, puppet: u64, message: Message, now_ms: u64) {
        match message {
            Message::Effect(effect) => match self.play_aligned_effect(build, puppet, &effect) {
                Ok(played) => {
                    self.retained_blocks.push_back((now_ms, played.block));
                    if let Some(handle) = played.handle {
                        count(&PLAYED_EFFECTS);
                        log_limited(&format!(
                            "effects: puppet {puppet:#x} effect {} -> handle {handle:#x} (sender {:#x})",
                            effect.effect, effect.handle
                        ));
                        if let Some(evicted) = self.live.track(handle, effect.handle, now_ms) {
                            self.finish_effects(build, &[evicted]);
                        }
                    } else {
                        count(&PLAY_FAILED);
                        log_limited(&format!(
                            "effects: puppet {puppet:#x} effect {} returned no handle",
                            effect.effect
                        ));
                    }
                }
                Err(error) => {
                    count(&PLAY_FAILED);
                    log_limited(&format!("effects: cannot play {} ({error})", effect.effect));
                }
            },
            Message::Sound(sound) => {
                let gain = distance_to_listener(build, puppet).map_or(1.0, sound_gain);
                if gain <= 0.0 {
                    count(&SILENCED_BY_DISTANCE);
                } else if play_sound(build, puppet, &sound.sound, gain) {
                    count(&PLAYED_SOUNDS);
                    log_limited(&format!(
                        "effects: puppet {puppet:#x} sound {}",
                        sound.sound
                    ));
                } else {
                    count(&PLAY_FAILED);
                    log_limited(&format!(
                        "effects: puppet sound {} returned nothing",
                        sound.sound
                    ));
                }
            }
            Message::WorldEffect(effect) => match spawn_world_effect(build, &effect) {
                Some(handle) => {
                    note_world_effect_shown(&effect.effect, true);
                    note_partner_hit_effect_played();
                    count(&PLAYED_EFFECTS);
                    log_limited(&format!(
                        "effects: world effect {} -> handle {handle:#x} (sender {:#x})",
                        effect.effect, effect.handle
                    ));
                    if let Some(evicted) = self.live.track(handle, effect.handle, now_ms) {
                        self.finish_effects(build, &[evicted]);
                    }
                    if let Some(look) = effect.look {
                        self.waiting_looks.push(WaitingLook {
                            handle,
                            name: effect.effect.clone(),
                            look,
                            frames_waited: 0,
                        });
                    }
                }
                None => {
                    note_world_effect_shown(&effect.effect, false);
                    count(&PLAY_FAILED);
                    log_limited(&format!(
                        "effects: world effect {} was not created",
                        effect.effect
                    ));
                }
            },
            Message::WorldEffectMove(moved) => {
                if let Some(handle) = self.live.own_handle_of(moved.handle) {
                    move_world_effect(build, handle, &moved.transform);
                }
            }
            Message::EffectStop(stop) => {
                if let Some(puppet_handle) = self.live.take_by_sender(stop.handle) {
                    self.finish_effects(build, &[puppet_handle]);
                    count(&PLAYED_STOPS);
                    log_line(&format!(
                        "effects: puppet stop sender handle {:#x} -> puppet handle {puppet_handle:#x}",
                        stop.handle
                    ));
                } else {
                    log_line(&format!(
                        "effects: stop for sender handle {:#x} found no puppet effect (already gone)",
                        stop.handle
                    ));
                }
            }
            _ => {}
        }
    }
}

pub fn tick(build: &VerifiedBuild) {
    let Some(net) = net::handle() else {
        return;
    };
    let active = net.view().partner_in_play();
    set_sending(active);
    if active {
        send_ended_effects(build);
    }
    let arrived = net.take_relayed(|message| {
        matches!(
            message,
            Message::Effect(_)
                | Message::Sound(_)
                | Message::EffectStop(_)
                | Message::BookEffect(_)
                | Message::BookSound(_)
                | Message::BulletEffect(_)
                | Message::WorldEffect(_)
                | Message::WorldEffectMove(_)
                | Message::VoiceCharacter(_)
        )
    });
    let arrived: Vec<Message> = arrived
        .into_iter()
        .filter(|message| {
            if let Message::VoiceCharacter(voice) = message {
                remember_partner_character(voice);
                return false;
            }
            true
        })
        .collect();
    let mut mirror = MIRROR.lock().unwrap_or_else(PoisonError::into_inner);
    let now = Instant::now();
    let now_ms = net_clock_ms();
    mirror.expire(build, now_ms);
    mirror.apply_waiting_looks(build);
    if active {
        send_local_character(build, now_ms);
        for message in arrived {
            if mirror.pending.len() >= MAX_PENDING_EVENTS {
                mirror.pending.pop_front();
                count(&DROPPED_WITHOUT_PUPPET);
            }
            mirror.pending.push_back((now, message));
        }
        play_due(build, &mut mirror, now, now_ms);
    } else {
        forget_sent_character();
        mirror.pending.clear();
        mirror.waiting_looks.clear();
        let all = mirror.live.take_all();
        mirror.finish_effects(build, &all);
    }
    if now_ms.saturating_sub(mirror.last_summary_ms) >= SUMMARY_INTERVAL_MS {
        mirror.last_summary_ms = now_ms;
        log_summary(mirror.live.len());
    }
}

fn play_due(build: &VerifiedBuild, mirror: &mut Mirror, now: Instant, now_ms: u64) {
    let puppet = first_actor_address()
        .filter(|actor| actor_is_live(&InProcessMemory::new(build), *actor).unwrap_or(false));
    let mut played = 0;
    while played < MAX_PUPPET_EFFECTS_PER_TICK
        && mirror
            .pending
            .front()
            .is_some_and(|(arrived, _)| now.saturating_duration_since(*arrived) >= PLAYBACK_DELAY)
    {
        let Some((_, message)) = mirror.pending.pop_front() else {
            break;
        };
        let (target, message) = match message {
            Message::BookEffect(effect) => (book_actor_address(), Message::Effect(effect)),
            Message::BookSound(sound) => (book_actor_address(), Message::Sound(sound)),
            Message::BulletEffect(bullet) => (
                book_actor_moved_to(build, bullet.position),
                Message::Effect(bullet.effect),
            ),
            Message::Sound(sound) => {
                if let Some(puppet) = puppet {
                    play_as_partner_character(build, puppet, &sound.sound);
                }
                (puppet, Message::Sound(sound))
            }
            other => (puppet, other),
        };
        match target {
            Some(target) => {
                mirror.play(build, target, message, now_ms);
                played += 1;
            }
            None => count(&DROPPED_WITHOUT_PUPPET),
        }
    }
}
