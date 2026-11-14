use replicant_game::bullet_flights::{FlightSample, MAX_EXTRAPOLATION_MS, SightedBullet};
use replicant_game::enemy_bullets::{
    CopyCorrection, CopyMotion, CopyMotionTally, EnemyBullet, EnemyBulletSender,
    SHOT_CORRECTION_BLEND_MS, scale_growth_of_radius, scaled_look, shot_scale_growth,
};
use replicant_game::offsets::{
    BULLET_HIT_SPHERE_RADIUS_OFFSET, NORMAL_BULLET_ATTACK_SETTINGS_POINTER_OFFSET,
    NORMAL_BULLET_HIT_RADIUS_GROWTH_RATE_OFFSET,
    NORMAL_BULLET_SETTINGS_EFFECT_SCALE_DIVISOR_OFFSET,
};
use replicant_proto::{BULLET_STYLE_FLOATS, BulletLook, EnemyId};

use crate::fake_memory::FakeMemory;

const WIDE_SHOT: u64 = 0x2_4000_0000;
const WIDE_SHOT_SETTINGS: u64 = 0x2_6000_0000;
const FIRST_WIDTH: f32 = 1.4;
const WIDTH_GROWTH_PER_SECOND: f32 = 24.0;
const ARRIVED_MS: u64 = 10_000;
const FRAME_MS: u64 = 16;

fn sample_at(x: f32, velocity_x: f32, received_ms: u64) -> FlightSample {
    let mut transform = [0.0_f32; 12];
    transform[0] = 1.0;
    transform[5] = 1.0;
    transform[10] = 1.0;
    transform[3] = x;
    FlightSample {
        transform,
        velocity: [velocity_x, 0.0, 0.0],
        received_ms,
    }
}

fn wide_shot_growth() -> f32 {
    scale_growth_of_radius(FIRST_WIDTH, WIDTH_GROWTH_PER_SECOND)
}

fn close(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

#[test]
fn a_wide_shot_of_the_run_grows_by_its_width_rate_over_its_width() {
    assert!(close(wide_shot_growth(), 24.0 / 1.4));
    assert_eq!(scale_growth_of_radius(0.0, WIDTH_GROWTH_PER_SECOND), 0.0);
    assert_eq!(scale_growth_of_radius(FIRST_WIDTH, 0.0), 0.0);
    assert_eq!(
        scale_growth_of_radius(f32::NAN, WIDTH_GROWTH_PER_SECOND),
        0.0
    );
}

#[test]
fn the_growth_is_read_from_the_game_only_when_the_effects_scale_with_the_width() {
    let mut memory = FakeMemory::new();
    memory.put(
        WIDE_SHOT + NORMAL_BULLET_ATTACK_SETTINGS_POINTER_OFFSET,
        &WIDE_SHOT_SETTINGS.to_le_bytes(),
    );
    memory.put(
        WIDE_SHOT + BULLET_HIT_SPHERE_RADIUS_OFFSET,
        &FIRST_WIDTH.to_le_bytes(),
    );
    memory.put(
        WIDE_SHOT + NORMAL_BULLET_HIT_RADIUS_GROWTH_RATE_OFFSET,
        &WIDTH_GROWTH_PER_SECOND.to_le_bytes(),
    );
    memory.put(
        WIDE_SHOT_SETTINGS + NORMAL_BULLET_SETTINGS_EFFECT_SCALE_DIVISOR_OFFSET,
        &0.0_f32.to_le_bytes(),
    );
    assert_eq!(shot_scale_growth(&memory, WIDE_SHOT), 0.0);
    memory.put(
        WIDE_SHOT_SETTINGS + NORMAL_BULLET_SETTINGS_EFFECT_SCALE_DIVISOR_OFFSET,
        &2.0_f32.to_le_bytes(),
    );
    assert!(close(
        shot_scale_growth(&memory, WIDE_SHOT),
        wide_shot_growth()
    ));
}

#[test]
fn the_host_sends_each_shot_s_growth_with_its_flight_and_knows_its_first_send() {
    let mut transform = [0.0_f32; 12];
    transform[0] = 1.0;
    let seen = EnemyBullet {
        enemy: EnemyId {
            group: 100,
            ordinal: 1,
        },
        bullet: SightedBullet {
            actor: WIDE_SHOT,
            slot: 140,
            hit_id: 7,
            transform,
            look: BulletLook {
                effect_ids: [1300, 0, 0, 0],
                styles: [[1.0; BULLET_STYLE_FLOATS]; 4],
            },
        },
        sprite: None,
        beam: None,
        scale_growth_per_second: wide_shot_growth(),
    };
    let mut sender = EnemyBulletSender::default();
    let sent = sender.update(&[seen], ARRIVED_MS);
    assert_eq!(sent.len(), 1);
    assert!(close(sent[0].scale_growth_per_second, wide_shot_growth()));
    assert_eq!(sender.started_in_last_update(), [140]);
    let moved = sender.update(&[seen], ARRIVED_MS + 50);
    assert_eq!(moved.len(), 1);
    assert!(sender.started_in_last_update().is_empty());
}

#[test]
fn a_wide_copy_grows_on_every_frame_between_messages() {
    let motion = CopyMotion::new(sample_at(0.0, 0.0, ARRIVED_MS), wide_shot_growth());
    let scales: Vec<f32> = (0..4)
        .map(|frame| motion.scale_factor(ARRIVED_MS + frame * FRAME_MS))
        .collect();
    assert!(scales.windows(2).all(|pair| pair[1] > pair[0]));
    let width_after_three_frames = FIRST_WIDTH * scales[3];
    assert!(close(
        width_after_three_frames,
        FIRST_WIDTH + WIDTH_GROWTH_PER_SECOND * 0.048
    ));
    assert!(motion.resizing(ARRIVED_MS));
}

#[test]
fn a_copy_that_keeps_its_size_is_not_restyled() {
    let motion = CopyMotion::new(sample_at(0.0, 0.0, ARRIVED_MS), 0.0);
    assert_eq!(motion.scale_factor(ARRIVED_MS + 40), 1.0);
    assert!(!motion.resizing(ARRIVED_MS + 40));
}

#[test]
fn a_copy_flies_on_with_its_velocity_and_stops_when_messages_stop() {
    let motion = CopyMotion::new(sample_at(5.0, 10.0, ARRIVED_MS), 0.0);
    assert!(close(motion.transform(ARRIVED_MS + 30)[3], 5.3));
    let held = motion.transform(ARRIVED_MS + MAX_EXTRAPOLATION_MS)[3];
    assert_eq!(
        motion.transform(ARRIVED_MS + MAX_EXTRAPOLATION_MS * 4)[3],
        held
    );
}

#[test]
fn a_message_s_place_correction_is_blended_out_instead_of_jumped() {
    let mut motion = CopyMotion::new(sample_at(0.0, 10.0, ARRIVED_MS), 0.0);
    let next_ms = ARRIVED_MS + 50;
    let drawn_before = motion.transform(next_ms)[3];
    let correction = motion.correct(sample_at(0.0, 10.0, next_ms), 0.0, None);
    assert!(close(correction.place, 0.5));
    assert!(close(motion.transform(next_ms)[3], drawn_before));
    let halfway = next_ms + SHOT_CORRECTION_BLEND_MS / 2;
    assert!(close(motion.transform(halfway)[3], 0.25 + 0.5));
    let done = next_ms + SHOT_CORRECTION_BLEND_MS;
    assert!(close(motion.transform(done)[3], 1.0));
}

#[test]
fn a_far_jump_is_taken_at_once() {
    let mut motion = CopyMotion::new(sample_at(0.0, 0.0, ARRIVED_MS), 0.0);
    let correction = motion.correct(sample_at(30.0, 0.0, ARRIVED_MS + 50), 0.0, None);
    assert!(close(correction.place, 30.0));
    assert!(close(motion.transform(ARRIVED_MS + 50)[3], 30.0));
}

#[test]
fn a_new_look_s_scale_is_blended_in_from_the_scale_drawn() {
    let growth = wide_shot_growth();
    let mut motion = CopyMotion::new(sample_at(0.0, 0.0, ARRIVED_MS), growth);
    let next_ms = ARRIVED_MS + 65;
    let drawn = motion.scale_factor(next_ms);
    // The host's look 65 ms on is a little larger than the copy guessed.
    let host_ratio = drawn * 1.05;
    let correction = motion.correct(sample_at(0.0, 0.0, next_ms), growth, Some(host_ratio));
    assert!(close(correction.scale_ratio, 1.0 / 1.05));
    assert!(close(motion.scale_factor(next_ms) * host_ratio, drawn));
    let settled = next_ms + SHOT_CORRECTION_BLEND_MS;
    let expected = 1.0 + growth * SHOT_CORRECTION_BLEND_MS as f32 / 1000.0;
    assert!(close(motion.scale_factor(settled), expected));
}

#[test]
fn a_move_without_a_look_keeps_the_copy_growing_from_where_it_is() {
    let growth = wide_shot_growth();
    let mut motion = CopyMotion::new(sample_at(0.0, 0.0, ARRIVED_MS), growth);
    let next_ms = ARRIVED_MS + 40;
    let drawn = motion.scale_factor(next_ms);
    let correction = motion.correct(sample_at(0.0, 0.0, next_ms), growth, None);
    assert_eq!(correction.scale_ratio, 1.0);
    assert!(close(motion.scale_factor(next_ms), drawn));
    assert!(motion.scale_factor(next_ms + FRAME_MS) > drawn);
}

#[test]
fn scaling_a_look_changes_only_the_scale_axes_of_drawn_slots() {
    let mut look = BulletLook {
        effect_ids: [1300, 0, 0, 0],
        styles: [[0.0; BULLET_STYLE_FLOATS]; 4],
    };
    look.styles[0] = [0.5, 0.6, 0.7, 1.0, 2.0, 2.0, 2.0, 9.0];
    let scaled = scaled_look(&look, 1.5);
    assert_eq!(scaled.styles[0], [0.5, 0.6, 0.7, 1.0, 3.0, 3.0, 3.0, 9.0]);
    assert_eq!(scaled.styles[1], look.styles[1]);
}

#[test]
fn the_copy_line_counts_frames_and_the_largest_corrections() {
    let mut tally = CopyMotionTally::default();
    assert_eq!(tally.take_line(), None);
    tally.note_frame(2, 1);
    tally.note_frame(3, 1);
    tally.note_correction(CopyCorrection {
        place: 0.4,
        scale_ratio: 0.9,
    });
    assert_eq!(
        tally.take_line().as_deref(),
        Some(
            "shot copies: last 1 s moved up to 3 copies on their own over 2 frames (up to 1 growing or resizing), 1 host corrections, largest 0.40 m and 10% in size"
        )
    );
    assert_eq!(tally.take_line(), None);
}
