use std::collections::HashMap;

use replicant_game::bullet_flights::SightedBullet;
use replicant_game::enemy_bullets::{
    BULLET_SPRITE_FRAME_COUNT, BeamEnd, ENEMY_BULLET_STALE_MS, EnemyBullet, EnemyBulletBoard,
    EnemyBulletCensus, EnemyBulletKinds, EnemyBulletSender, EnemyBulletStep, FlightRefusals,
    LaserBeam, LaserBeamTally, MAX_ENEMY_BULLET_MOVES_SENT_PER_SCAN,
    MAX_SHOWN_BULLET_EFFECTS_LOGGED, MAX_SHOWN_ENEMY_BULLETS, MAX_SIGHTED_ENEMY_BULLETS,
    MAX_SIGHTED_ENEMY_SPRITE_BULLETS, MAX_SPRITE_BULLET_KINDS_LOGGED,
    SCRIPT_BULLET_LASER_FLAG_OFFSET, ShownEffectTally, SpriteBulletKinds, UncopiedScriptBullet,
    UncopiedTally, UncopiedWhy, bullets_of_enemies, first_sprite_frame, longest_axis, look_resized,
    look_scale, next_sprite_frame, scan_enemy_bullets,
};
use replicant_game::offsets::{
    ACTOR_ANCHOR_OFFSET_OFFSET, ACTOR_SELF_HANDLE_OFFSET, ACTOR_TRANSFORM_BLOCK_BYTES,
    ACTOR_TRANSFORM_BLOCK_OFFSET, BULLET_SECONDS_BEFORE_START_OFFSET,
    EFFECT_INSTANCE_MATRIX_OFFSET, EFFECT_INSTANCE_STYLE_OFFSET, EFFECT_POOL_GENERATION_OFFSET,
    EFFECT_POOL_RVA, EFFECT_POOL_SLOT_STRIDE, HANDLE_POOL_GENERATIONS_RVA,
    HANDLE_POOL_POINTERS_RVA, HANDLE_POOL_SLOT_COUNT, NORMAL_BULLET_EFFECT_HANDLES_OFFSET,
    NORMAL_BULLET_HIT_ID_OFFSET, NORMAL_BULLET_OWNER_HANDLE_OFFSET,
    NORMAL_BULLET_RECORD_EFFECT_IDS_OFFSET, NORMAL_BULLET_RECORD_POINTER_OFFSET,
    NORMAL_BULLET_STATE_OFFSET, NORMAL_BULLET_VTABLE_RVA,
    SCRIPT_BULLET_EFFECT_OBJECT_EFFECT_ID_OFFSET, SCRIPT_BULLET_EFFECT_OBJECT_HANDLE_OFFSET,
    SCRIPT_BULLET_EFFECT_OBJECT_POINTER_OFFSET, SCRIPT_BULLET_LOOP_EFFECT_VTABLE_RVA,
    SCRIPT_BULLET_SENIOR_HANDLE_OFFSET, SCRIPT_BULLET_SPRITE_ALPHA_OFFSET,
    SCRIPT_BULLET_SPRITE_ANGLE_RADIANS_OFFSET, SCRIPT_BULLET_SPRITE_EFFECT_VTABLE_RVA,
    SCRIPT_BULLET_SPRITE_KIND_OFFSET, SCRIPT_BULLET_SPRITE_SIZE_OFFSET,
    SCRIPT_BULLET_TO_SENIOR_EFFECT_LENGTH_OFFSET, SCRIPT_BULLET_TO_SENIOR_EFFECT_VTABLE_RVA,
    SCRIPT_BULLET_VTABLE_RVA, TRACE_BULLET_VTABLE_RVA,
};
use replicant_proto::{
    BULLET_STYLE_FLOATS, BulletFlight, BulletLook, BulletSprite, EnemyBulletFlight, EnemyId,
};

use crate::fake_memory::{FAKE_MODULE_BASE, FakeMemory};

const PLAYER: u64 = 0x2_0000_0000;
const SHADE: u64 = 0x2_3000_0000;
const OTHER_SHADE: u64 = 0x2_3100_0000;
const BULLET_HEAP: u64 = 0x2_4000_0000;
const BULLET_STRIDE: u64 = 0x2_0000;
const RECORD_HEAP: u64 = 0x2_6000_0000;
const EFFECT_HEAP: u64 = 0x2_8000_0000;
const PLAYER_SLOT: u32 = 5;
const SHADE_SLOT: u32 = 6;
const OTHER_SHADE_SLOT: u32 = 7;
const FIRST_BULLET_SLOT: u32 = 100;
const GENERATION: u32 = 9;
const EFFECT_GENERATION: i32 = 3;
const EFFECT_IDS: [i32; 4] = [0x1F7, 0x1FB, 0, 0];
const EFFECT_OBJECT_HEAP: u64 = 0x2_A000_0000;
const LOOP_EFFECT_ID: i32 = 0x4F;
const LOOP_EFFECT_STYLE: [f32; 8] = [0.9, 0.1, 0.2, 1.0, 2.0, 2.0, 2.0, 2.0];
const LOOP_EFFECT_X: f32 = 41.5;
const INVALID_EFFECT_HANDLE: u32 = 0xFFFF_E000;
const SPRITE: BulletSprite = BulletSprite {
    kind: 1,
    size: 0.75,
    alpha: 1.0,
    angle: 0.5,
};
const SHADE_ID: EnemyId = EnemyId {
    group: 3,
    ordinal: 1,
};
const OTHER_SHADE_ID: EnemyId = EnemyId {
    group: 3,
    ordinal: 4,
};

fn pool_entry(memory: &mut FakeMemory, slot: u32, actor: u64) {
    memory.put_at_rva(HANDLE_POOL_POINTERS_RVA + slot * 8, &actor.to_le_bytes());
    memory.put_at_rva(
        HANDLE_POOL_GENERATIONS_RVA + slot * 4,
        &GENERATION.to_le_bytes(),
    );
}

fn handle_bytes(slot: u32) -> Vec<u8> {
    let mut bytes = slot.to_le_bytes().to_vec();
    bytes.extend_from_slice(&GENERATION.to_le_bytes());
    bytes
}

fn world() -> FakeMemory {
    let mut memory = FakeMemory::new();
    let empty_table = vec![0_u8; HANDLE_POOL_SLOT_COUNT as usize * 8];
    memory.put_at_rva(HANDLE_POOL_POINTERS_RVA, &empty_table);
    pool_entry(&mut memory, PLAYER_SLOT, PLAYER);
    pool_entry(&mut memory, SHADE_SLOT, SHADE);
    pool_entry(&mut memory, OTHER_SHADE_SLOT, OTHER_SHADE);
    memory
}

fn shared_shades() -> HashMap<u64, EnemyId> {
    HashMap::from([(SHADE, SHADE_ID), (OTHER_SHADE, OTHER_SHADE_ID)])
}

fn live_effect(memory: &mut FakeMemory, effect_slot: u32) -> u32 {
    let instance = EFFECT_HEAP + u64::from(effect_slot) * 0x1000;
    let pool_slot = EFFECT_POOL_RVA + effect_slot * EFFECT_POOL_SLOT_STRIDE as u32;
    memory.put_at_rva(pool_slot, &instance.to_le_bytes());
    memory.put_at_rva(
        pool_slot + EFFECT_POOL_GENERATION_OFFSET as u32,
        &EFFECT_GENERATION.to_le_bytes(),
    );
    (EFFECT_GENERATION.cast_unsigned() << 13) | effect_slot
}

fn bullet(memory: &mut FakeMemory, index: u32, owner_slot: u32) -> u64 {
    let actor = BULLET_HEAP + u64::from(index) * BULLET_STRIDE;
    let vtable = FAKE_MODULE_BASE + u64::from(NORMAL_BULLET_VTABLE_RVA);
    memory.put(actor, &vtable.to_le_bytes());
    memory.put(
        actor + NORMAL_BULLET_OWNER_HANDLE_OFFSET,
        &handle_bytes(owner_slot),
    );
    let slot = FIRST_BULLET_SLOT + index;
    memory.put(actor + ACTOR_SELF_HANDLE_OFFSET, &handle_bytes(slot));
    pool_entry(memory, slot, actor);
    memory.put(actor + NORMAL_BULLET_STATE_OFFSET, &0_i32.to_le_bytes());
    memory.put(
        actor + NORMAL_BULLET_HIT_ID_OFFSET,
        &(2000 + index as i32).to_le_bytes(),
    );
    let record = RECORD_HEAP + u64::from(index) * 0x1000;
    memory.put(
        actor + NORMAL_BULLET_RECORD_POINTER_OFFSET,
        &record.to_le_bytes(),
    );
    for (lane, id) in EFFECT_IDS.iter().enumerate() {
        memory.put(
            record + NORMAL_BULLET_RECORD_EFFECT_IDS_OFFSET + lane as u64 * 4,
            &id.to_le_bytes(),
        );
    }
    for lane in 0..4_u64 {
        let handle = if lane == 0 {
            live_effect(memory, index * 4)
        } else {
            0xFFFF_E000
        };
        memory.put(
            actor + NORMAL_BULLET_EFFECT_HANDLES_OFFSET + lane * 4,
            &handle.to_le_bytes(),
        );
    }
    memory.put(
        actor + ACTOR_TRANSFORM_BLOCK_OFFSET,
        &[0_u8; ACTOR_TRANSFORM_BLOCK_BYTES],
    );
    actor
}

fn floats_as_bytes(floats: &[f32]) -> Vec<u8> {
    floats
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn pooled_bullet_of_class(
    memory: &mut FakeMemory,
    index: u32,
    owner_slot: u32,
    vtable_rva: u32,
) -> u64 {
    let actor = BULLET_HEAP + u64::from(index) * BULLET_STRIDE;
    let vtable = FAKE_MODULE_BASE + u64::from(vtable_rva);
    memory.put(actor, &vtable.to_le_bytes());
    memory.put(
        actor + NORMAL_BULLET_OWNER_HANDLE_OFFSET,
        &handle_bytes(owner_slot),
    );
    let slot = FIRST_BULLET_SLOT + index;
    memory.put(actor + ACTOR_SELF_HANDLE_OFFSET, &handle_bytes(slot));
    pool_entry(memory, slot, actor);
    memory.put(actor + NORMAL_BULLET_STATE_OFFSET, &0_i32.to_le_bytes());
    memory.put(
        actor + NORMAL_BULLET_HIT_ID_OFFSET,
        &(3000 + index as i32).to_le_bytes(),
    );
    memory.put(
        actor + SCRIPT_BULLET_EFFECT_OBJECT_POINTER_OFFSET,
        &0_u64.to_le_bytes(),
    );
    actor
}

fn effect_object(
    memory: &mut FakeMemory,
    bullet_actor: u64,
    index: u32,
    vtable_rva: u32,
    effect_id: i32,
    handle: u32,
) {
    let object = EFFECT_OBJECT_HEAP + u64::from(index) * 0x100;
    let vtable = FAKE_MODULE_BASE + u64::from(vtable_rva);
    memory.put(object, &vtable.to_le_bytes());
    memory.put(
        object + SCRIPT_BULLET_EFFECT_OBJECT_HANDLE_OFFSET,
        &handle.to_le_bytes(),
    );
    memory.put(
        object + SCRIPT_BULLET_EFFECT_OBJECT_EFFECT_ID_OFFSET,
        &effect_id.to_le_bytes(),
    );
    memory.put(
        bullet_actor + SCRIPT_BULLET_EFFECT_OBJECT_POINTER_OFFSET,
        &object.to_le_bytes(),
    );
}

fn script_bullet_with_effect(
    memory: &mut FakeMemory,
    index: u32,
    owner_slot: u32,
    effect_class_rva: u32,
) -> u64 {
    let actor = pooled_bullet_of_class(memory, index, owner_slot, SCRIPT_BULLET_VTABLE_RVA);
    let handle = live_effect(memory, index * 4);
    let instance = EFFECT_HEAP + u64::from(index * 4) * 0x1000;
    let mut transform = [0.0_f32; 12];
    transform[3] = LOOP_EFFECT_X;
    memory.put(
        instance + EFFECT_INSTANCE_MATRIX_OFFSET,
        &floats_as_bytes(&transform),
    );
    memory.put(
        instance + EFFECT_INSTANCE_STYLE_OFFSET,
        &floats_as_bytes(&LOOP_EFFECT_STYLE),
    );
    effect_object(
        memory,
        actor,
        index,
        effect_class_rva,
        LOOP_EFFECT_ID,
        handle,
    );
    actor
}

fn sprite_bullet(memory: &mut FakeMemory, index: u32, owner_slot: u32) -> u64 {
    let actor = pooled_bullet_of_class(memory, index, owner_slot, SCRIPT_BULLET_VTABLE_RVA);
    let object = EFFECT_OBJECT_HEAP + u64::from(index) * 0x100;
    let vtable = FAKE_MODULE_BASE + u64::from(SCRIPT_BULLET_SPRITE_EFFECT_VTABLE_RVA);
    memory.put(object, &vtable.to_le_bytes());
    memory.put(
        object + SCRIPT_BULLET_SPRITE_ALPHA_OFFSET,
        &SPRITE.alpha.to_le_bytes(),
    );
    memory.put(object + SCRIPT_BULLET_SPRITE_KIND_OFFSET, &[SPRITE.kind]);
    memory.put(
        object + SCRIPT_BULLET_SPRITE_ANGLE_RADIANS_OFFSET,
        &SPRITE.angle.to_le_bytes(),
    );
    memory.put(
        object + SCRIPT_BULLET_SPRITE_SIZE_OFFSET,
        &SPRITE.size.to_le_bytes(),
    );
    memory.put(
        actor + SCRIPT_BULLET_EFFECT_OBJECT_POINTER_OFFSET,
        &object.to_le_bytes(),
    );
    memory.put(
        actor + BULLET_SECONDS_BEFORE_START_OFFSET,
        &0.0_f32.to_le_bytes(),
    );
    let mut transform = [0.0_f32; 12];
    transform[0] = 1.0;
    transform[5] = 1.0;
    transform[10] = 1.0;
    transform[3] = 10.0;
    transform[7] = 20.0;
    transform[11] = 30.0;
    memory.put(
        actor + ACTOR_TRANSFORM_BLOCK_OFFSET,
        &floats_as_bytes(&transform),
    );
    memory.put(
        actor + ACTOR_ANCHOR_OFFSET_OFFSET,
        &floats_as_bytes(&[0.0, 1.5, 0.0]),
    );
    actor
}

fn volley(count: u16) -> Vec<EnemyBullet> {
    (0..count)
        .map(|index| sighting(SHADE_ID, 200 + index, i32::from(index) + 1, 0.0))
        .collect()
}

fn sighting(enemy: EnemyId, slot: u16, hit_id: i32, x: f32) -> EnemyBullet {
    let mut transform = [0.0_f32; 12];
    transform[3] = x;
    EnemyBullet {
        enemy,
        bullet: SightedBullet {
            actor: 0x1000,
            slot,
            hit_id,
            transform,
            look: BulletLook {
                effect_ids: EFFECT_IDS,
                styles: [[0.0; BULLET_STYLE_FLOATS]; 4],
            },
        },
        sprite: None,
        beam: None,
        scale_growth_per_second: 0.0,
    }
}

fn sprite_sighting(slot: u16, x: f32) -> EnemyBullet {
    EnemyBullet {
        sprite: Some(SPRITE),
        ..sighting(SHADE_ID, slot, 1, x)
    }
}

fn flight(instance: u16, with_look: bool, ended: bool) -> EnemyBulletFlight {
    EnemyBulletFlight {
        enemy: SHADE_ID,
        flight: BulletFlight {
            instance,
            transform: [0.0; 12],
            velocity: [0.0; 3],
            look: with_look.then_some(BulletLook {
                effect_ids: EFFECT_IDS,
                styles: [[0.0; BULLET_STYLE_FLOATS]; 4],
            }),
            ended,
        },
        sprite: None,
        scale_growth_per_second: 0.0,
    }
}

fn sprite_flight(instance: u16) -> EnemyBulletFlight {
    EnemyBulletFlight {
        sprite: Some(SPRITE),
        ..flight(instance, false, false)
    }
}

#[test]
fn only_bullets_fired_by_shared_enemies_are_sighted_with_their_enemy() {
    let mut memory = world();
    let shade_shot = bullet(&mut memory, 0, SHADE_SLOT);
    bullet(&mut memory, 1, PLAYER_SLOT);
    let other_shot = bullet(&mut memory, 2, OTHER_SHADE_SLOT);
    let finished = bullet(&mut memory, 3, SHADE_SLOT);
    memory.put(finished + NORMAL_BULLET_STATE_OFFSET, &1_i32.to_le_bytes());
    let sighted = bullets_of_enemies(&memory, &shared_shades()).unwrap();
    assert_eq!(sighted.len(), 2);
    assert_eq!(sighted[0].enemy, SHADE_ID);
    assert_eq!(sighted[0].bullet.actor, shade_shot);
    assert_eq!(sighted[0].bullet.look.effect_ids, EFFECT_IDS);
    assert_eq!(sighted[1].enemy, OTHER_SHADE_ID);
    assert_eq!(sighted[1].bullet.actor, other_shot);
}

#[test]
fn no_enemy_bullets_are_sighted_without_shared_enemies_or_past_the_limit() {
    let mut memory = world();
    for index in 0..MAX_SIGHTED_ENEMY_BULLETS as u32 + 4 {
        bullet(&mut memory, index, SHADE_SLOT);
    }
    assert!(
        bullets_of_enemies(&memory, &HashMap::new())
            .unwrap()
            .is_empty()
    );
    let scan = scan_enemy_bullets(&memory, &shared_shades()).unwrap();
    assert_eq!(scan.bullets.len(), MAX_SIGHTED_ENEMY_BULLETS);
    assert_eq!(scan.census.past_the_limit, 4);
}

#[test]
fn a_script_bullet_drawn_by_a_looping_effect_is_sighted_where_its_effect_is() {
    let mut memory = world();
    let looping = script_bullet_with_effect(
        &mut memory,
        0,
        SHADE_SLOT,
        SCRIPT_BULLET_LOOP_EFFECT_VTABLE_RVA,
    );
    let to_senior = script_bullet_with_effect(
        &mut memory,
        1,
        OTHER_SHADE_SLOT,
        SCRIPT_BULLET_TO_SENIOR_EFFECT_VTABLE_RVA,
    );
    script_bullet_with_effect(
        &mut memory,
        2,
        PLAYER_SLOT,
        SCRIPT_BULLET_LOOP_EFFECT_VTABLE_RVA,
    );
    let scan = scan_enemy_bullets(&memory, &shared_shades()).unwrap();
    assert_eq!(scan.bullets.len(), 2);
    assert_eq!(scan.bullets[0].enemy, SHADE_ID);
    assert_eq!(scan.bullets[0].bullet.actor, looping);
    assert_eq!(scan.bullets[0].bullet.hit_id, 3000);
    assert_eq!(scan.bullets[0].bullet.transform[3], LOOP_EFFECT_X);
    assert_eq!(
        scan.bullets[0].bullet.look.effect_ids,
        [LOOP_EFFECT_ID, 0, 0, 0]
    );
    assert_eq!(scan.bullets[0].bullet.look.styles[0], LOOP_EFFECT_STYLE);
    assert_eq!(scan.bullets[1].enemy, OTHER_SHADE_ID);
    assert_eq!(scan.bullets[1].bullet.actor, to_senior);
    assert_eq!(scan.census.script_with_effect, 2);
}

#[test]
fn a_script_bullet_with_nothing_to_copy_is_counted_and_not_sighted() {
    let mut memory = world();
    pooled_bullet_of_class(&mut memory, 0, SHADE_SLOT, SCRIPT_BULLET_VTABLE_RVA);
    let sprite = pooled_bullet_of_class(&mut memory, 1, SHADE_SLOT, SCRIPT_BULLET_VTABLE_RVA);
    effect_object(
        &mut memory,
        sprite,
        1,
        SCRIPT_BULLET_SPRITE_EFFECT_VTABLE_RVA,
        0,
        1.0_f32.to_bits(),
    );
    let gone = pooled_bullet_of_class(&mut memory, 2, SHADE_SLOT, SCRIPT_BULLET_VTABLE_RVA);
    effect_object(
        &mut memory,
        gone,
        2,
        SCRIPT_BULLET_LOOP_EFFECT_VTABLE_RVA,
        LOOP_EFFECT_ID,
        INVALID_EFFECT_HANDLE,
    );
    let bad_id = script_bullet_with_effect(
        &mut memory,
        3,
        SHADE_SLOT,
        SCRIPT_BULLET_LOOP_EFFECT_VTABLE_RVA,
    );
    let bad_id_effect = live_effect(&mut memory, 12);
    effect_object(
        &mut memory,
        bad_id,
        3,
        SCRIPT_BULLET_LOOP_EFFECT_VTABLE_RVA,
        -1,
        bad_id_effect,
    );
    let unknown_class = script_bullet_with_effect(
        &mut memory,
        4,
        SHADE_SLOT,
        SCRIPT_BULLET_LOOP_EFFECT_VTABLE_RVA,
    );
    let unknown_class_effect = live_effect(&mut memory, 16);
    effect_object(
        &mut memory,
        unknown_class,
        4,
        NORMAL_BULLET_VTABLE_RVA,
        LOOP_EFFECT_ID,
        unknown_class_effect,
    );
    let finished = script_bullet_with_effect(
        &mut memory,
        5,
        SHADE_SLOT,
        SCRIPT_BULLET_LOOP_EFFECT_VTABLE_RVA,
    );
    memory.put(finished + NORMAL_BULLET_STATE_OFFSET, &1_i32.to_le_bytes());
    pooled_bullet_of_class(&mut memory, 6, SHADE_SLOT, TRACE_BULLET_VTABLE_RVA);
    let scan = scan_enemy_bullets(&memory, &shared_shades()).unwrap();
    assert!(scan.bullets.is_empty());
    assert_eq!(
        scan.census,
        EnemyBulletCensus {
            script_sprite_not_drawn: 1,
            script_without_effect: 4,
            trace: 1,
            ..EnemyBulletCensus::default()
        }
    );
}

#[test]
fn a_sprite_drawn_script_bullet_is_sighted_at_its_anchor_with_what_the_batch_draws() {
    let mut memory = world();
    let sprite = sprite_bullet(&mut memory, 0, SHADE_SLOT);
    sprite_bullet(&mut memory, 1, PLAYER_SLOT);
    let scan = scan_enemy_bullets(&memory, &shared_shades()).unwrap();
    assert_eq!(scan.bullets.len(), 1);
    let sighted = scan.bullets[0];
    assert_eq!(sighted.enemy, SHADE_ID);
    assert_eq!(sighted.bullet.actor, sprite);
    assert_eq!(sighted.bullet.hit_id, 3000);
    assert_eq!(sighted.sprite, Some(SPRITE));
    assert_eq!(sighted.bullet.transform[3], 10.0);
    assert_eq!(sighted.bullet.transform[7], 21.5);
    assert_eq!(sighted.bullet.transform[11], 30.0);
    assert_eq!(
        scan.census,
        EnemyBulletCensus {
            script_sprite: 1,
            ..EnemyBulletCensus::default()
        }
    );
}

#[test]
fn a_sprite_bullet_the_game_does_not_draw_yet_is_counted_and_not_sighted() {
    let mut memory = world();
    let waiting = sprite_bullet(&mut memory, 0, SHADE_SLOT);
    memory.put(
        waiting + BULLET_SECONDS_BEFORE_START_OFFSET,
        &0.4_f32.to_le_bytes(),
    );
    sprite_bullet(&mut memory, 1, SHADE_SLOT);
    let sizeless_object = EFFECT_OBJECT_HEAP + 0x100;
    memory.put(
        sizeless_object + SCRIPT_BULLET_SPRITE_SIZE_OFFSET,
        &0.0_f32.to_le_bytes(),
    );
    sprite_bullet(&mut memory, 2, SHADE_SLOT);
    let unknown_kind_object = EFFECT_OBJECT_HEAP + 0x200;
    memory.put(unknown_kind_object + SCRIPT_BULLET_SPRITE_KIND_OFFSET, &[9]);
    let scan = scan_enemy_bullets(&memory, &shared_shades()).unwrap();
    assert!(scan.bullets.is_empty());
    assert_eq!(scan.census.script_sprite_not_drawn, 3);
    assert_eq!(scan.census.script_sprite, 0);
}

#[test]
fn sprite_bullets_have_their_own_limit_beside_the_effect_drawn_ones() {
    let mut memory = world();
    let effect_drawn = MAX_SIGHTED_ENEMY_BULLETS as u32 + 2;
    for index in 0..effect_drawn {
        bullet(&mut memory, index, SHADE_SLOT);
    }
    for index in 0..MAX_SIGHTED_ENEMY_SPRITE_BULLETS as u32 + 3 {
        sprite_bullet(&mut memory, effect_drawn + index, SHADE_SLOT);
    }
    let scan = scan_enemy_bullets(&memory, &shared_shades()).unwrap();
    let sprites = scan
        .bullets
        .iter()
        .filter(|seen| seen.sprite.is_some())
        .count();
    assert_eq!(sprites, MAX_SIGHTED_ENEMY_SPRITE_BULLETS);
    assert_eq!(scan.bullets.len() - sprites, MAX_SIGHTED_ENEMY_BULLETS);
    assert_eq!(scan.census.past_the_limit, 5);
}

#[test]
fn a_sprite_bullet_is_sent_with_its_sprite_on_every_step_and_never_a_look() {
    let mut sender = EnemyBulletSender::default();
    for scan in 0..12_u64 {
        let flights = sender.update(&[sprite_sighting(120, scan as f32)], 1000 + scan * 50);
        assert_eq!(flights.len(), 1);
        assert_eq!(flights[0].sprite, Some(SPRITE));
        assert_eq!(flights[0].flight.look, None);
        assert_eq!(flights[0].validate(), Ok(()));
    }
    let ended = sender.update(&[], 2000);
    assert_eq!(ended.len(), 1);
    assert!(ended[0].flight.ended);
    assert_eq!(ended[0].sprite, None);
}

#[test]
fn the_guest_starts_a_sprite_bullet_from_any_step_that_carries_its_sprite() {
    let mut board = EnemyBulletBoard::default();
    assert_eq!(board.note(&sprite_flight(7), 0), EnemyBulletStep::Start);
    assert_eq!(board.note(&sprite_flight(7), 10), EnemyBulletStep::Move);
    assert_eq!(
        board.note(&flight(7, false, true), 20),
        EnemyBulletStep::Stop
    );
    assert!(board.is_empty());
}

#[test]
fn each_sprite_bullet_kind_is_logged_once_and_only_a_few_kinds() {
    let mut kinds = SpriteBulletKinds::default();
    assert!(kinds.first_sighting(&SPRITE));
    assert!(!kinds.first_sighting(&SPRITE));
    assert!(!kinds.first_sighting(&BulletSprite {
        alpha: 0.25,
        angle: 2.0,
        ..SPRITE
    }));
    let more = (0..MAX_SPRITE_BULLET_KINDS_LOGGED)
        .filter(|index| {
            kinds.first_sighting(&BulletSprite {
                size: 2.0 + *index as f32,
                ..SPRITE
            })
        })
        .count();
    assert_eq!(more, MAX_SPRITE_BULLET_KINDS_LOGGED - 1);
}

#[test]
fn a_shown_sprite_loops_through_the_frames_of_the_sprite_sheet() {
    assert_eq!(first_sprite_frame(5), 5);
    assert!(first_sprite_frame(4095) < BULLET_SPRITE_FRAME_COUNT);
    assert_eq!(next_sprite_frame(0), 1);
    assert_eq!(next_sprite_frame(BULLET_SPRITE_FRAME_COUNT - 1), 0);
}

#[test]
fn the_census_keeps_the_highest_count_of_each_class() {
    let mut most = EnemyBulletCensus {
        normal: 3,
        script_sprite: 1,
        ..EnemyBulletCensus::default()
    };
    assert!(!most.is_empty());
    most.keep_highest(&EnemyBulletCensus {
        normal: 1,
        script_with_effect: 30,
        ..EnemyBulletCensus::default()
    });
    assert_eq!(
        most,
        EnemyBulletCensus {
            normal: 3,
            script_with_effect: 30,
            script_sprite: 1,
            ..EnemyBulletCensus::default()
        }
    );
    most.keep_highest(&EnemyBulletCensus {
        script_sprite_not_drawn: 7,
        ..EnemyBulletCensus::default()
    });
    assert_eq!(most.script_sprite_not_drawn, 7);
    assert!(EnemyBulletCensus::default().is_empty());
}

#[test]
fn a_volley_is_sent_a_bounded_number_per_scan_and_every_bullet_starts_with_its_look() {
    let mut sender = EnemyBulletSender::default();
    let bullets = volley(MAX_ENEMY_BULLET_MOVES_SENT_PER_SCAN as u16 * 2 + 5);
    let mut started = Vec::new();
    for scan in 0..3_u64 {
        let flights = sender.update(&bullets, 1000 + scan * 50);
        assert!(flights.len() <= MAX_ENEMY_BULLET_MOVES_SENT_PER_SCAN);
        for sent in flights {
            assert!(!sent.flight.ended);
            if !started.contains(&sent.flight.instance) {
                assert!(sent.flight.look.is_some());
                started.push(sent.flight.instance);
            }
        }
    }
    assert_eq!(started.len(), bullets.len());
    assert_eq!(sender.len(), bullets.len());
}

#[test]
fn bullets_in_the_air_take_turns_so_the_one_that_waited_longest_moves_next() {
    let mut sender = EnemyBulletSender::default();
    let bullets = volley(MAX_ENEMY_BULLET_MOVES_SENT_PER_SCAN as u16 * 2);
    let first: Vec<u16> = sender
        .update(&bullets, 1000)
        .iter()
        .map(|sent| sent.flight.instance)
        .collect();
    let second: Vec<u16> = sender
        .update(&bullets, 1050)
        .iter()
        .map(|sent| sent.flight.instance)
        .collect();
    let third = sender.update(&bullets, 1100);
    assert!(first.iter().all(|instance| !second.contains(instance)));
    assert_eq!(
        third
            .iter()
            .map(|sent| sent.flight.instance)
            .collect::<Vec<u16>>(),
        first
    );
    assert!(third.iter().all(|sent| sent.flight.look.is_none()));
}

#[test]
fn a_bullet_that_ends_before_the_guest_heard_of_it_sends_no_end() {
    let mut sender = EnemyBulletSender::default();
    let bullets = volley(MAX_ENEMY_BULLET_MOVES_SENT_PER_SCAN as u16 + 3);
    let told: Vec<u16> = sender
        .update(&bullets, 1000)
        .iter()
        .map(|sent| sent.flight.instance)
        .collect();
    let ended = sender.update(&[], 1050);
    assert_eq!(ended.len(), MAX_ENEMY_BULLET_MOVES_SENT_PER_SCAN);
    assert!(ended.iter().all(|sent| sent.flight.ended));
    assert!(
        ended
            .iter()
            .all(|sent| told.contains(&sent.flight.instance))
    );
    assert!(sender.is_empty());
}

#[test]
fn a_bullet_the_guest_follows_gets_its_look_again_every_few_moves() {
    let mut sender = EnemyBulletSender::default();
    let with_look: Vec<bool> = (0..21_u64)
        .map(|scan| {
            sender.update(&[sighting(SHADE_ID, 120, 1, scan as f32)], 1000 + scan * 50)[0]
                .flight
                .look
                .is_some()
        })
        .collect();
    let expected: Vec<bool> = (0..21).map(|scan| scan % 10 == 0).collect();
    assert_eq!(with_look, expected);
}

fn widening_sighting(slot: u16, scale: f32) -> EnemyBullet {
    let mut seen = sighting(SHADE_ID, slot, 1, 0.0);
    seen.bullet.look.styles[0] = [1.0, 1.0, 1.0, 1.0, scale, scale, scale, 0.0];
    seen
}

#[test]
fn a_widening_bullet_sends_its_look_on_every_move_while_its_effects_grow() {
    let mut sender = EnemyBulletSender::default();
    let scales = [1.4, 2.6, 3.8, 5.0, 5.0, 5.0];
    let mut with_look = Vec::new();
    let mut resized = Vec::new();
    for (scan, scale) in (0_u64..).zip(scales) {
        let sent = sender.update(&[widening_sighting(140, scale)], 1000 + scan * 50);
        with_look.push(sent[0].flight.look.is_some());
        resized.push(sender.resized_in_last_update().to_vec());
    }
    assert_eq!(with_look, [true, true, true, true, false, false]);
    assert_eq!(
        resized,
        [
            vec![],
            vec![(140, 2.6)],
            vec![(140, 3.8)],
            vec![(140, 5.0)],
            vec![],
            vec![]
        ]
    );
}

#[test]
fn a_look_is_resized_only_when_a_scale_changes_and_its_scale_is_the_largest_styled_axis() {
    let small = widening_sighting(1, 1.4).bullet.look;
    let mut recoloured = small;
    recoloured.styles[0][0] = 0.2;
    let wide = widening_sighting(1, 9.8).bullet.look;
    assert!(!look_resized(&small, &recoloured));
    assert!(look_resized(&small, &wide));
    assert!((look_scale(&wide) - 9.8).abs() < f32::EPSILON);
    let mut unstyled = wide;
    unstyled.styles[0][3] = 0.0;
    assert!(look_scale(&unstyled).abs() < f32::EPSILON);
}

#[test]
fn a_sent_enemy_bullet_carries_its_enemy_from_first_flight_to_end() {
    let mut sender = EnemyBulletSender::default();
    let first = sender.update(&[sighting(SHADE_ID, 120, 1, 0.0)], 1000);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].enemy, SHADE_ID);
    assert!(first[0].flight.look.is_some());
    let moved = sender.update(&[sighting(SHADE_ID, 120, 1, 5.0)], 1050);
    assert_eq!(moved[0].enemy, SHADE_ID);
    assert!(!moved[0].flight.ended);
    let ended = sender.update(&[], 1100);
    assert_eq!(ended.len(), 1);
    assert!(ended[0].flight.ended);
    assert_eq!(ended[0].enemy, SHADE_ID);
    assert!(sender.is_empty());
}

#[test]
fn a_recycled_slot_ends_the_old_enemys_bullet_and_starts_the_new_enemys() {
    let mut sender = EnemyBulletSender::default();
    sender.update(&[sighting(SHADE_ID, 120, 1, 0.0)], 1000);
    let flights = sender.update(&[sighting(OTHER_SHADE_ID, 120, 2, 9.0)], 1050);
    assert_eq!(flights.len(), 2);
    assert!(flights[0].flight.ended);
    assert_eq!(flights[0].enemy, SHADE_ID);
    assert!(!flights[1].flight.ended);
    assert_eq!(flights[1].enemy, OTHER_SHADE_ID);
}

#[test]
fn ending_all_enemy_bullets_ends_each_with_its_enemy() {
    let mut sender = EnemyBulletSender::default();
    sender.update(
        &[
            sighting(SHADE_ID, 120, 1, 0.0),
            sighting(OTHER_SHADE_ID, 121, 2, 0.0),
        ],
        1000,
    );
    let ended = sender.end_all();
    assert_eq!(ended.len(), 2);
    assert!(ended.iter().all(|flight| flight.flight.ended));
    assert_eq!(ended[0].enemy, SHADE_ID);
    assert_eq!(ended[1].enemy, OTHER_SHADE_ID);
    assert!(sender.end_all().is_empty());
}

#[test]
fn the_guest_starts_an_enemy_bullet_only_with_its_look_then_moves_and_stops_it() {
    let mut board = EnemyBulletBoard::default();
    assert_eq!(
        board.note(&flight(7, false, false), 0),
        EnemyBulletStep::Ignore
    );
    assert_eq!(
        board.note(&flight(7, true, false), 10),
        EnemyBulletStep::Start
    );
    assert_eq!(
        board.note(&flight(7, false, false), 20),
        EnemyBulletStep::Move
    );
    assert_eq!(
        board.note(&flight(7, false, true), 30),
        EnemyBulletStep::Stop
    );
    assert_eq!(
        board.note(&flight(7, false, true), 40),
        EnemyBulletStep::Ignore
    );
    assert!(board.is_empty());
}

#[test]
fn the_guest_shows_no_more_than_the_limit_and_drops_quiet_bullets() {
    let mut board = EnemyBulletBoard::default();
    for instance in 0..MAX_SHOWN_ENEMY_BULLETS as u16 {
        assert_eq!(
            board.note(&flight(instance, true, false), 0),
            EnemyBulletStep::Start
        );
    }
    assert_eq!(
        board.note(&flight(999, true, false), 0),
        EnemyBulletStep::Full
    );
    board.note(&flight(0, false, false), ENEMY_BULLET_STALE_MS);
    let stale = board.take_stale(ENEMY_BULLET_STALE_MS);
    assert_eq!(stale.len(), MAX_SHOWN_ENEMY_BULLETS - 1);
    assert!(!stale.contains(&0));
    assert_eq!(board.take_all(), vec![0]);
}

#[test]
fn each_enemy_bullet_kind_is_logged_once() {
    let mut kinds = EnemyBulletKinds::default();
    assert!(kinds.first_sighting(EFFECT_IDS));
    assert!(!kinds.first_sighting(EFFECT_IDS));
    assert!(kinds.first_sighting([0x245, 0, 0, 0]));
}

#[test]
fn each_script_bullet_with_nothing_to_copy_says_why_and_whether_it_is_a_laser() {
    let mut memory = world();
    let laser = pooled_bullet_of_class(&mut memory, 0, SHADE_SLOT, SCRIPT_BULLET_VTABLE_RVA);
    memory.put(laser + SCRIPT_BULLET_LASER_FLAG_OFFSET, &[0]);
    let gone = pooled_bullet_of_class(&mut memory, 2, SHADE_SLOT, SCRIPT_BULLET_VTABLE_RVA);
    memory.put(gone + SCRIPT_BULLET_LASER_FLAG_OFFSET, &[1]);
    effect_object(
        &mut memory,
        gone,
        2,
        SCRIPT_BULLET_LOOP_EFFECT_VTABLE_RVA,
        LOOP_EFFECT_ID,
        INVALID_EFFECT_HANDLE,
    );
    let bad_id = script_bullet_with_effect(
        &mut memory,
        3,
        OTHER_SHADE_SLOT,
        SCRIPT_BULLET_LOOP_EFFECT_VTABLE_RVA,
    );
    let bad_id_effect = live_effect(&mut memory, 12);
    effect_object(
        &mut memory,
        bad_id,
        3,
        SCRIPT_BULLET_LOOP_EFFECT_VTABLE_RVA,
        0x5000,
        bad_id_effect,
    );
    let unknown_class = script_bullet_with_effect(
        &mut memory,
        4,
        SHADE_SLOT,
        SCRIPT_BULLET_LOOP_EFFECT_VTABLE_RVA,
    );
    let unknown_class_effect = live_effect(&mut memory, 16);
    effect_object(
        &mut memory,
        unknown_class,
        4,
        NORMAL_BULLET_VTABLE_RVA,
        LOOP_EFFECT_ID,
        unknown_class_effect,
    );
    script_bullet_with_effect(
        &mut memory,
        5,
        SHADE_SLOT,
        SCRIPT_BULLET_LOOP_EFFECT_VTABLE_RVA,
    );
    let scan = scan_enemy_bullets(&memory, &shared_shades()).unwrap();
    assert_eq!(scan.bullets.len(), 1);
    assert_eq!(
        scan.uncopied,
        vec![
            UncopiedScriptBullet {
                enemy: SHADE_ID,
                why: UncopiedWhy::NoEffectObject,
                laser: true,
            },
            UncopiedScriptBullet {
                enemy: SHADE_ID,
                why: UncopiedWhy::EffectGone {
                    effect_id: LOOP_EFFECT_ID
                },
                laser: false,
            },
            UncopiedScriptBullet {
                enemy: OTHER_SHADE_ID,
                why: UncopiedWhy::EffectIdOutOfRange { effect_id: 0x5000 },
                laser: false,
            },
            UncopiedScriptBullet {
                enemy: SHADE_ID,
                why: UncopiedWhy::UnknownEffectObject {
                    vtable_rva: NORMAL_BULLET_VTABLE_RVA
                },
                laser: false,
            },
        ]
    );
}

#[test]
fn an_uncopied_kind_is_named_once_and_the_summary_keeps_the_most_at_once() {
    let laser = UncopiedScriptBullet {
        enemy: SHADE_ID,
        why: UncopiedWhy::NoEffectObject,
        laser: true,
    };
    let mut tally = UncopiedTally::default();
    let first = tally.add_scan(&[laser, laser]);
    assert_eq!(
        first,
        vec![
            "enemy bullets: first script bullet with no effect to copy from enemy 3:1: with no effect object, spawned as a laser"
                .to_owned()
        ]
    );
    assert!(tally.add_scan(&[laser, laser, laser]).is_empty());
    assert!(tally.add_scan(&[laser]).is_empty());
    assert_eq!(
        tally.take_summary().as_deref(),
        Some(
            "enemy bullets: last 5 s script bullets with no effect to copy, most at once: 3 from 3:1 with no effect object, spawned as a laser"
        )
    );
    assert_eq!(tally.take_summary(), None);
}

fn place_actor(memory: &mut FakeMemory, actor: u64, position: [f32; 3]) {
    let mut transform = [0.0_f32; 12];
    transform[0] = 1.0;
    transform[5] = 1.0;
    transform[10] = 1.0;
    transform[3] = position[0];
    transform[7] = position[1];
    transform[11] = position[2];
    memory.put(
        actor + ACTOR_TRANSFORM_BLOCK_OFFSET,
        &floats_as_bytes(&transform),
    );
}

fn laser_beam_world(far_end_spawned_as_laser: bool) -> (FakeMemory, u64) {
    let mut memory = world();
    let beam = script_bullet_with_effect(
        &mut memory,
        0,
        SHADE_SLOT,
        SCRIPT_BULLET_TO_SENIOR_EFFECT_VTABLE_RVA,
    );
    let object = EFFECT_OBJECT_HEAP;
    memory.put(
        object + SCRIPT_BULLET_TO_SENIOR_EFFECT_LENGTH_OFFSET,
        &2.0_f32.to_le_bytes(),
    );
    let far_end = pooled_bullet_of_class(&mut memory, 1, SHADE_SLOT, SCRIPT_BULLET_VTABLE_RVA);
    let flag = u8::from(!far_end_spawned_as_laser);
    memory.put(far_end + SCRIPT_BULLET_LASER_FLAG_OFFSET, &[flag]);
    memory.put(
        beam + SCRIPT_BULLET_SENIOR_HANDLE_OFFSET,
        &handle_bytes(FIRST_BULLET_SLOT + 1),
    );
    place_actor(&mut memory, beam, [0.0, 2.0, 0.0]);
    place_actor(&mut memory, far_end, [0.0, 2.0, 30.0]);
    (memory, beam)
}

fn beam_sighting(slot: u16, stretch: f32) -> EnemyBullet {
    let mut seen = sighting(SHADE_ID, slot, 1, 5.0);
    seen.bullet.transform[0] = 1.0;
    seen.bullet.transform[5] = 1.0;
    seen.bullet.transform[10] = stretch;
    seen.bullet.look.effect_ids = [8059, 0, 0, 0];
    seen.beam = Some(LaserBeam {
        far_end: BeamEnd::SpawnedAsLaser,
        length: stretch * 2.0,
        effect_length: 2.0,
    });
    seen
}

#[test]
fn a_script_bullet_whose_effect_is_stretched_to_its_senior_is_sighted_as_a_laser_beam() {
    let (memory, beam) = laser_beam_world(true);
    let scan = scan_enemy_bullets(&memory, &shared_shades()).unwrap();
    assert_eq!(scan.bullets.len(), 1);
    assert_eq!(scan.bullets[0].bullet.actor, beam);
    assert_eq!(
        scan.bullets[0].beam,
        Some(LaserBeam {
            far_end: BeamEnd::SpawnedAsLaser,
            length: 30.0,
            effect_length: 2.0,
        })
    );
    assert_eq!(scan.uncopied.len(), 1);
    assert!(scan.uncopied[0].laser);
    let (plain_memory, _) = laser_beam_world(false);
    let plain = scan_enemy_bullets(&plain_memory, &shared_shades()).unwrap();
    assert_eq!(
        plain.bullets[0].beam.map(|beam| beam.far_end),
        Some(BeamEnd::PlainScriptBullet)
    );
}

#[test]
fn a_beam_whose_senior_is_gone_is_still_a_beam_and_a_looping_effect_is_not_one() {
    let (mut memory, beam) = laser_beam_world(true);
    memory.put(beam + SCRIPT_BULLET_SENIOR_HANDLE_OFFSET, &[0xFF; 8]);
    let scan = scan_enemy_bullets(&memory, &shared_shades()).unwrap();
    assert_eq!(
        scan.bullets[0].beam.map(|beam| beam.far_end),
        Some(BeamEnd::Gone)
    );
    let mut looping_world = world();
    script_bullet_with_effect(
        &mut looping_world,
        0,
        SHADE_SLOT,
        SCRIPT_BULLET_LOOP_EFFECT_VTABLE_RVA,
    );
    let looping = scan_enemy_bullets(&looping_world, &shared_shades()).unwrap();
    assert_eq!(looping.bullets[0].beam, None);
}

#[test]
fn the_longest_axis_of_a_bullet_is_one_unscaled_and_the_stretch_for_a_beam() {
    let mut transform = [0.0_f32; 12];
    transform[0] = 1.0;
    transform[5] = 1.0;
    transform[10] = 1.0;
    transform[3] = 500.0;
    assert_eq!(longest_axis(&transform), 1.0);
    transform[2] = 18.0;
    transform[6] = 0.0;
    transform[10] = 24.0;
    assert_eq!(longest_axis(&transform), 30.0);
}

#[test]
fn a_stretched_beam_is_sent_on_every_scan_and_passes_the_wire_check() {
    let mut sender = EnemyBulletSender::default();
    for scan in 0..5_u64 {
        let flights = sender.update(&[beam_sighting(40, 30.0 + scan as f32)], scan * 50);
        assert_eq!(flights.len(), 1);
        assert_eq!(flights[0].validate(), Ok(()));
    }
}

#[test]
fn the_beam_tally_names_each_beam_kind_once_and_counts_beams_sent() {
    let mut tally = LaserBeamTally::default();
    assert_eq!(tally.take_summary(), None);
    let first = tally.add_scan(&[beam_sighting(40, 15.0), sighting(SHADE_ID, 41, 1, 0.0)]);
    assert_eq!(
        first,
        vec![
            "enemy bullets: first laser beam from enemy 3:1: effect 8059 stretched to a bullet spawned as a laser, 30.0 units long over an effect length of 2.00 (longest axis 15.0)"
                .to_owned()
        ]
    );
    assert!(tally.add_scan(&[beam_sighting(40, 20.0)]).is_empty());
    tally.note_sent(40);
    tally.note_sent(40);
    tally.note_sent(41);
    assert_eq!(
        tally.take_summary().as_deref(),
        Some(
            "enemy bullets: last 5 s laser beams: 1 in the air (most at once 1), 1 of them sent in 2 flight messages, longest 40.0 units (longest axis 20.0)"
        )
    );
    assert_eq!(tally.take_summary(), None);
    assert!(tally.add_scan(&[beam_sighting(42, 1.0)]).is_empty());
}

#[test]
fn flights_that_fail_the_wire_check_are_counted_by_reason() {
    let mut refusals = FlightRefusals::default();
    assert_eq!(refusals.take_summary(), None);
    refusals.note("bullet transform is out of range".to_owned());
    refusals.note("bullet transform is out of range".to_owned());
    refusals.note("bullet style is out of range".to_owned());
    assert_eq!(
        refusals.take_summary().as_deref(),
        Some(
            "enemy bullets: last 5 s 3 flights failed the wire check and were not sent: bullet style is out of range x1, bullet transform is out of range x2"
        )
    );
    assert_eq!(refusals.take_summary(), None);
}

#[test]
fn the_guest_counts_shown_bullets_by_effect_with_their_longest_axis() {
    let mut tally = ShownEffectTally::default();
    assert_eq!(tally.take_summary(), None);
    let beam = beam_sighting(40, 12.0).bullet.transform;
    tally.note_start(8059, &beam);
    tally.note_move(8059, &beam_sighting(40, 31.5).bullet.transform);
    tally.note_move(8059, &beam);
    assert_eq!(
        tally.take_summary().as_deref(),
        Some(
            "enemy bullets: last 5 s shown by effect id: 8059: 1 started, 2 moves, longest axis 31.5"
        )
    );
    for effect_id in 0..=MAX_SHOWN_BULLET_EFFECTS_LOGGED as i32 {
        tally.note_start(effect_id, &beam);
    }
    let line = tally.take_summary().unwrap();
    assert!(line.ends_with(" and 1 more"), "{line}");
}
