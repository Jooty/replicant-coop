use std::collections::BTreeMap;
use std::sync::{LazyLock, Mutex, PoisonError};

use replicant_game::bullet_flights::{CueOnCopy, FlightSample, ShotCueBoard};
use replicant_game::enemy_bullets::{
    CopyMotion, CopyMotionTally, EnemyBulletBoard, EnemyBulletKinds, EnemyBulletStep,
    MAX_FOLLOWING_EFFECTS_PER_COPY, SHOT_CUE_SUMMARY_INTERVAL_MS, ShotCueTally, ShownEffectTally,
    SpriteBulletKinds, first_sprite_frame, look_resized, look_scale, next_sprite_frame,
    scaled_look,
};
use replicant_proto::{
    BULLET_EFFECT_SLOTS, BULLET_TRANSFORM_FLOATS, BulletFlight, BulletLook, BulletSprite,
    EnemyBulletFlight, Message, ShotCue, ShotCueKind,
};

use crate::build_gate::VerifiedBuild;
use crate::bullets::{create_effect, end_effect, place_effect, style_effect};
use crate::diagnostics::log_line;
use crate::enemies::enemy_id_text;
use crate::enemy_bullets::ENEMY_BULLET_SUMMARY_INTERVAL_MS;
use crate::enemy_bullets::shot_sounds::{move_shot_sound, play_shot_sound, stop_shot_sound};
use crate::enemy_bullets::sprite_draw::{
    SpriteToDraw, set_sprites_to_draw, take_sprite_draw_counts,
};
use crate::net::{NetThread, net_clock_ms};

const SHOT_CUE_REFUSED_BY_THE_GAME: &str = "the game refused";
const SMALLEST_SCALE_CHANGE_RESTYLED: f32 = 0.002;

static SHOWN: LazyLock<Mutex<Shown>> = LazyLock::new(Mutex::default);

#[derive(Debug)]
struct ShownSprite {
    sprite: BulletSprite,
    frame: u16,
}

#[derive(Debug)]
struct ShownBullet {
    handles: [Option<u32>; BULLET_EFFECT_SLOTS],
    effect_id: i32,
    look: Option<BulletLook>,
    sprite: Option<ShownSprite>,
    motion: CopyMotion,
    drawn_scale: f32,
    following: Vec<u32>,
    lasting_sound: Option<u64>,
}

#[derive(Debug, Default)]
struct Shown {
    board: EnemyBulletBoard,
    live: BTreeMap<u16, ShownBullet>,
    kinds: EnemyBulletKinds,
    sprite_kinds: SpriteBulletKinds,
    shown_effects: ShownEffectTally,
    started: u32,
    started_sprites: u32,
    ended: u32,
    timed_out: u32,
    refused: u32,
    last_summary_ms: u64,
    cues: ShotCueBoard,
    cue_tally: ShotCueTally,
    last_cue_summary_ms: u64,
    motion_tally: CopyMotionTally,
}

impl Shown {
    fn apply(&mut self, build: &VerifiedBuild, arrived: EnemyBulletFlight, now_ms: u64) {
        let flight = arrived.flight;
        match self.board.note(&arrived, now_ms) {
            EnemyBulletStep::Start => {
                if let Some(sprite) = arrived.sprite {
                    self.start_sprite(arrived, sprite, now_ms);
                } else if let Some(look) = flight.look {
                    self.start(build, arrived, look, now_ms);
                }
            }
            EnemyBulletStep::Move => {
                if let Some(live) = self.live.get_mut(&flight.instance) {
                    let resized = match (live.look, flight.look) {
                        (Some(shown), Some(now)) if look_resized(&shown, &now) => {
                            Some((shown, now))
                        }
                        _ => None,
                    };
                    // A widening shot: the host's game grows its effects with its hit radius.
                    let rescaled = resized.and_then(|(shown, now)| {
                        let before = look_scale(&shown);
                        (before > 0.0).then(|| look_scale(&now) / before)
                    });
                    let correction = live.motion.correct(
                        sample_of(&flight, now_ms),
                        arrived.scale_growth_per_second,
                        rescaled,
                    );
                    self.motion_tally.note_correction(correction);
                    if let (Some(shown), Some(sprite)) = (live.sprite.as_mut(), arrived.sprite) {
                        shown.sprite = sprite;
                    }
                    if live.sprite.is_none() {
                        self.shown_effects
                            .note_move(live.effect_id, &flight.transform);
                    }
                    if let Some((_, now)) = resized {
                        live.look = Some(now);
                        // Forces the next frame to restyle the effects at the new look.
                        live.drawn_scale = f32::NAN;
                        self.cue_tally
                            .note_resize(flight.instance, look_scale(&now));
                    }
                }
            }
            EnemyBulletStep::Stop => {
                if self.stop(build, flight.instance) {
                    self.ended += 1;
                }
            }
            EnemyBulletStep::Full => self.refused += 1,
            EnemyBulletStep::Ignore => {}
        }
    }

    fn start(
        &mut self,
        build: &VerifiedBuild,
        arrived: EnemyBulletFlight,
        look: BulletLook,
        now_ms: u64,
    ) {
        let flight = arrived.flight;
        let mut handles = [None; BULLET_EFFECT_SLOTS];
        for (index, handle) in handles.iter_mut().enumerate() {
            *handle = create_effect(build, look.effect_ids[index], &flight.transform);
            if let Some(handle) = *handle {
                style_effect(build, handle, &look.styles[index]);
            }
        }
        if handles.iter().all(Option::is_none) {
            self.board.forget(flight.instance);
            self.refused += 1;
            return;
        }
        if self.kinds.first_sighting(look.effect_ids) {
            log_line(&format!(
                "enemy bullets: kind {:?} from enemy {} shown",
                look.effect_ids,
                enemy_id_text(arrived.enemy)
            ));
        }
        if arrived.scale_growth_per_second > 0.0 {
            log_line(&format!(
                "wide shot: {} from enemy {} shown at scale {:.1}, growing {:.1} times its size a second",
                flight.instance,
                enemy_id_text(arrived.enemy),
                look_scale(&look),
                arrived.scale_growth_per_second
            ));
        }
        self.started += 1;
        self.shown_effects
            .note_start(look.effect_ids[0], &flight.transform);
        self.live.insert(
            flight.instance,
            ShownBullet {
                handles,
                effect_id: look.effect_ids[0],
                look: Some(look),
                sprite: None,
                motion: CopyMotion::new(
                    sample_of(&flight, now_ms),
                    arrived.scale_growth_per_second,
                ),
                drawn_scale: 1.0,
                following: Vec::new(),
                lasting_sound: None,
            },
        );
    }

    fn start_sprite(&mut self, arrived: EnemyBulletFlight, sprite: BulletSprite, now_ms: u64) {
        let flight = arrived.flight;
        if self.sprite_kinds.first_sighting(&sprite) {
            log_line(&format!(
                "enemy bullets: sprite kind {} of size {} from enemy {} goes to the game's sprite batch on each draw",
                sprite.kind,
                sprite.size,
                enemy_id_text(arrived.enemy)
            ));
        }
        self.started += 1;
        self.started_sprites += 1;
        self.live.insert(
            flight.instance,
            ShownBullet {
                handles: [None; BULLET_EFFECT_SLOTS],
                effect_id: 0,
                look: None,
                sprite: Some(ShownSprite {
                    sprite,
                    frame: first_sprite_frame(flight.instance),
                }),
                motion: CopyMotion::new(sample_of(&flight, now_ms), 0.0),
                drawn_scale: 1.0,
                following: Vec::new(),
                lasting_sound: None,
            },
        );
    }

    fn stop(&mut self, build: &VerifiedBuild, instance: u16) -> bool {
        let Some(shown) = self.live.remove(&instance) else {
            return false;
        };
        let last = shown.motion.transform(net_clock_ms());
        self.cues.note_stop(instance, last);
        for handle in shown
            .handles
            .into_iter()
            .chain(shown.following.into_iter().map(Some))
            .flatten()
        {
            end_effect(build, handle);
        }
        if let Some(sound) = shown.lasting_sound {
            stop_shot_sound(build, sound);
        }
        true
    }

    fn live_transform(&self, instance: u16, now_ms: u64) -> Option<[f32; BULLET_TRANSFORM_FLOATS]> {
        Some(self.live.get(&instance)?.motion.transform(now_ms))
    }

    fn apply_cue(&mut self, build: &VerifiedBuild, cue: ShotCue, now_ms: u64) {
        let live = cue
            .instance
            .is_some_and(|instance| self.live.contains_key(&instance));
        match self.cues.note(cue, live) {
            CueOnCopy::Follow { instance } => self.follow_cue(build, instance, cue, now_ms),
            CueOnCopy::End { instance } => self.end_cue(build, instance, cue.kind),
            CueOnCopy::Place { transform } => self.make_one_shot(build, &cue, &transform),
            CueOnCopy::Wait => {}
            CueOnCopy::Drop { why } => self.cue_tally.note_dropped(why),
        }
    }

    fn follow_cue(&mut self, build: &VerifiedBuild, instance: u16, cue: ShotCue, now_ms: u64) {
        let Some(transform) = self.live_transform(instance, now_ms) else {
            return;
        };
        let Some(shown) = self.live.get_mut(&instance) else {
            return;
        };
        if cue.kind == ShotCueKind::LastingSound {
            if let Some(sound) = shown.lasting_sound.take() {
                stop_shot_sound(build, sound);
            }
            shown.lasting_sound =
                play_shot_sound(build, cue.id, translation_of(&transform), cue.sound_levels);
            if shown.lasting_sound.is_none() {
                self.cue_tally.note_dropped(SHOT_CUE_REFUSED_BY_THE_GAME);
                return;
            }
        } else {
            let Some(handle) = create_effect(build, effect_id_of(&cue), &transform) else {
                self.cue_tally.note_dropped(SHOT_CUE_REFUSED_BY_THE_GAME);
                return;
            };
            style_effect(build, handle, &cue.style);
            if shown.following.len() >= MAX_FOLLOWING_EFFECTS_PER_COPY {
                end_effect(build, shown.following.remove(0));
            }
            shown.following.push(handle);
        }
        self.cue_tally.note_cue(cue.instance, cue.kind);
    }

    fn end_cue(&mut self, build: &VerifiedBuild, instance: u16, kind: ShotCueKind) {
        let Some(shown) = self.live.get_mut(&instance) else {
            return;
        };
        if kind == ShotCueKind::EndLastingSound {
            if let Some(sound) = shown.lasting_sound.take() {
                stop_shot_sound(build, sound);
            }
        } else {
            for handle in shown.following.drain(..) {
                end_effect(build, handle);
            }
        }
        self.cue_tally.note_cue(Some(instance), kind);
    }

    fn make_one_shot(
        &mut self,
        build: &VerifiedBuild,
        cue: &ShotCue,
        transform: &[f32; BULLET_TRANSFORM_FLOATS],
    ) {
        let made = if cue.kind == ShotCueKind::OneShotSound {
            play_shot_sound(build, cue.id, translation_of(transform), cue.sound_levels).is_some()
        } else {
            create_effect(build, effect_id_of(cue), transform)
                .inspect(|handle| style_effect(build, *handle, &cue.style))
                .is_some()
        };
        if made {
            self.cue_tally.note_cue(cue.instance, cue.kind);
        } else {
            self.cue_tally.note_dropped(SHOT_CUE_REFUSED_BY_THE_GAME);
        }
    }

    fn make_due_one_shots(&mut self, build: &VerifiedBuild, now_ms: u64) {
        let live: Vec<(u16, [f32; BULLET_TRANSFORM_FLOATS])> = self
            .live
            .keys()
            .filter_map(|instance| Some((*instance, self.live_transform(*instance, now_ms)?)))
            .collect();
        let due = self.cues.take_due(|instance| {
            live.iter()
                .find(|(known, _)| *known == instance)
                .map(|(_, transform)| *transform)
        });
        for (cue, transform) in due {
            self.make_one_shot(build, &cue, &transform);
        }
    }

    fn follow(&mut self, build: &VerifiedBuild, now_ms: u64) {
        let mut sprites = Vec::new();
        let mut resizing = 0;
        for shown in self.live.values_mut() {
            let transform = shown.motion.transform(now_ms);
            if let Some(look) = shown.look
                && (shown.motion.resizing(now_ms) || shown.drawn_scale.is_nan())
            {
                resizing += 1;
                let factor = shown.motion.scale_factor(now_ms);
                if shown.drawn_scale.is_nan()
                    || (factor - shown.drawn_scale).abs() >= SMALLEST_SCALE_CHANGE_RESTYLED
                {
                    let drawn = scaled_look(&look, factor);
                    for (handle, style) in shown.handles.iter().zip(&drawn.styles) {
                        if let Some(handle) = handle {
                            style_effect(build, *handle, style);
                        }
                    }
                    shown.drawn_scale = factor;
                }
            }
            for handle in shown
                .handles
                .into_iter()
                .chain(shown.following.iter().copied().map(Some))
                .flatten()
            {
                place_effect(build, handle, &transform);
            }
            if let Some(sound) = shown.lasting_sound
                && !move_shot_sound(build, sound, translation_of(&transform))
            {
                shown.lasting_sound = None;
            }
            if let Some(sprite) = shown.sprite.as_mut() {
                sprite.frame = next_sprite_frame(sprite.frame);
                sprites.push(SpriteToDraw {
                    position: [transform[3], transform[7], transform[11]],
                    sprite: sprite.sprite,
                    frame: sprite.frame,
                });
            }
        }
        set_sprites_to_draw(sprites);
        self.motion_tally.note_frame(self.live.len(), resizing);
    }

    fn summarize_cues(&mut self, now_ms: u64) {
        if now_ms.saturating_sub(self.last_cue_summary_ms) < SHOT_CUE_SUMMARY_INTERVAL_MS {
            return;
        }
        self.last_cue_summary_ms = now_ms;
        if let Some(line) = self.cue_tally.take_line("made", self.live.len()) {
            log_line(&line);
        }
        if let Some(line) = self.motion_tally.take_line() {
            log_line(&line);
        }
    }

    fn summarize(&mut self, now_ms: u64) {
        if now_ms.saturating_sub(self.last_summary_ms) < ENEMY_BULLET_SUMMARY_INTERVAL_MS {
            return;
        }
        self.last_summary_ms = now_ms;
        self.summarize_sprite_draws();
        if let Some(line) = self.shown_effects.take_summary() {
            log_line(&line);
        }
        if self.started + self.ended + self.timed_out + self.refused == 0 {
            return;
        }
        log_line(&format!(
            "enemy bullets: last 5 s showed {} enemy bullets ({} of them sprites), {} ended, {} timed out, {} could not be drawn, {} in the air",
            self.started,
            self.started_sprites,
            self.ended,
            self.timed_out,
            self.refused,
            self.live.len()
        ));
        self.started = 0;
        self.started_sprites = 0;
        self.ended = 0;
        self.timed_out = 0;
        self.refused = 0;
    }

    fn summarize_sprite_draws(&self) {
        let counts = take_sprite_draw_counts();
        let sprites_in_the_air = self
            .live
            .values()
            .filter(|shown| shown.sprite.is_some())
            .count();
        if sprites_in_the_air == 0 && counts.draws == 0 {
            return;
        }
        // Handed but not taken: off screen, too near the camera, or the batch was full.
        log_line(&format!(
            "enemy bullets: last 5 s the bullet draw ran {} times with host sprites ({} found no sprite batch), handed the batch {} sprites and it took {}, {} sprites in the air now",
            counts.draws,
            counts.draws_without_a_batch,
            counts.handed,
            counts.taken,
            sprites_in_the_air
        ));
    }
}

pub(super) fn show_enemy_bullets(build: &VerifiedBuild, net: &NetThread, in_play: bool) {
    let arrived = net.take_relayed(|message| {
        matches!(message, Message::EnemyBulletFlight(_) | Message::ShotCue(_))
    });
    let mut shown = SHOWN.lock().unwrap_or_else(PoisonError::into_inner);
    if !in_play {
        for instance in shown.board.take_all() {
            shown.stop(build, instance);
        }
        let _ = shown.cues.take_due(|_| None);
        set_sprites_to_draw(Vec::new());
        return;
    }
    let now_ms = net_clock_ms();
    for message in arrived {
        match message {
            Message::EnemyBulletFlight(flight) => shown.apply(build, flight, now_ms),
            Message::ShotCue(cue) => shown.apply_cue(build, cue, now_ms),
            _ => {}
        }
    }
    for instance in shown.board.take_stale(now_ms) {
        if shown.stop(build, instance) {
            shown.timed_out += 1;
        }
    }
    shown.make_due_one_shots(build, now_ms);
    shown.follow(build, now_ms);
    shown.summarize(now_ms);
    shown.summarize_cues(now_ms);
}

fn sample_of(flight: &BulletFlight, now_ms: u64) -> FlightSample {
    FlightSample {
        transform: flight.transform,
        velocity: flight.velocity,
        received_ms: now_ms,
    }
}

fn translation_of(transform: &[f32; BULLET_TRANSFORM_FLOATS]) -> [f32; 3] {
    [transform[3], transform[7], transform[11]]
}

fn effect_id_of(cue: &ShotCue) -> i32 {
    i32::try_from(cue.id).unwrap_or(i32::MAX)
}
