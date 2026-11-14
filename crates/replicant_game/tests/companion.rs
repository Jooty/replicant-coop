use replicant_game::GameMemory;
use replicant_game::Position;
use replicant_game::enemies::{
    CompanionControl, CompanionControlLog, CompanionGuestInputs, CompanionOwnLogicCause,
    companion_control, companion_control_text, companion_hit_counts, companion_name, enemy_state,
    host_sends_companion, read_companions,
};
use replicant_game::offsets::{
    ACTOR_CLASS_ID_OFFSET, ACTOR_LIFE_STATE_OFFSET, ACTOR_SELF_HANDLE_OFFSET,
    ACTOR_SPAWN_SERIAL_OFFSET, ACTOR_TRANSFORM_BLOCK_BYTES, ACTOR_TRANSFORM_BLOCK_OFFSET,
    HANDLE_POOL_GENERATIONS_RVA, HANDLE_POOL_POINTERS_RVA, PARTNER_ACTIVE_FLAG_OFFSET,
    PARTNER_KAINE_ACTOR_RVA, PARTNER_SCRIPT_MODE_OFFSET, PARTNER_THIRD_MEMBER_ACTOR_RVA,
    PARTNER_VARIANT_POINTER_RVA,
};
use replicant_game::puppet::PuppetPose;
use replicant_proto::{EnemyId, Message, encode};

use crate::fake_memory::FakeMemory;

const VARIANT: u64 = 0x9_0000_0000;

const FOLLOWING: CompanionGuestInputs = CompanionGuestInputs {
    host_motion_known: Some(true),
    snap_enabled: true,
    in_event_scene: false,
    script_mode: false,
};

fn put_partner(memory: &mut FakeMemory, actor: u64, slot: u32, active: bool, script_mode: i32) {
    let mut handle = [0_u8; 8];
    handle[..4].copy_from_slice(&slot.to_le_bytes());
    handle[4..].copy_from_slice(&1_u32.to_le_bytes());
    memory.put(actor + ACTOR_SELF_HANDLE_OFFSET, &handle);
    memory.put_at_rva(HANDLE_POOL_POINTERS_RVA + slot * 8, &actor.to_le_bytes());
    memory.put_at_rva(HANDLE_POOL_GENERATIONS_RVA + slot * 4, &1_u32.to_le_bytes());
    memory.put(actor + ACTOR_SPAWN_SERIAL_OFFSET, &7_i32.to_le_bytes());
    memory.put(actor + ACTOR_CLASS_ID_OFFSET, &2_i32.to_le_bytes());
    memory.put(actor + ACTOR_LIFE_STATE_OFFSET, &0_i32.to_le_bytes());
    memory.put(actor + PARTNER_ACTIVE_FLAG_OFFSET, &[u8::from(active)]);
    memory.put(
        actor + PARTNER_SCRIPT_MODE_OFFSET,
        &script_mode.to_le_bytes(),
    );
    let pose = PuppetPose {
        position: Position {
            x: 4.0,
            y: 5.0,
            z: 6.0,
        },
        yaw: 0.25,
    };
    memory.put(
        actor + ACTOR_TRANSFORM_BLOCK_OFFSET,
        &pose.creation_matrix()[..ACTOR_TRANSFORM_BLOCK_BYTES],
    );
}

fn kaine(memory: &FakeMemory) -> u64 {
    memory.module_base() + u64::from(PARTNER_KAINE_ACTOR_RVA)
}

fn third(memory: &FakeMemory) -> u64 {
    memory.module_base() + u64::from(PARTNER_THIRD_MEMBER_ACTOR_RVA)
}

#[test]
fn no_companion_is_read_when_none_is_active() {
    let mut memory = FakeMemory::new();
    assert!(read_companions(&memory, |_| Some((1, 1))).is_empty());
    let actor = kaine(&memory);
    put_partner(&mut memory, actor, 9, false, 0);
    assert!(read_companions(&memory, |_| Some((1, 1))).is_empty());
}

#[test]
fn an_active_companion_is_read_with_its_fixed_id_place_and_life() {
    let mut memory = FakeMemory::new();
    let actor = kaine(&memory);
    put_partner(&mut memory, actor, 9, true, 0);
    let found = read_companions(&memory, |asked| (asked == actor).then_some((80, 100)));
    assert_eq!(found.len(), 1);
    let companion = found[0];
    assert_eq!(companion.id, EnemyId::companion(0));
    assert!(!companion.script_mode);
    assert_eq!(companion.sighting.actor, actor);
    assert_eq!(companion.sighting.spawn_serial, 7);
    assert_eq!(companion.sighting.class_id, 2);
    assert_eq!(companion.sighting.position, [4.0, 5.0, 6.0]);
    assert!((companion.sighting.yaw_radians - 0.25).abs() < 1e-5);
    assert_eq!(
        (companion.sighting.hp, companion.sighting.max_hp),
        (80, 100)
    );
}

#[test]
fn each_companion_slot_keeps_its_own_id_whichever_are_active() {
    let mut memory = FakeMemory::new();
    let (first, second) = (kaine(&memory), third(&memory));
    put_partner(&mut memory, first, 9, false, 0);
    put_partner(&mut memory, second, 10, true, 0);
    put_partner(&mut memory, VARIANT, 11, true, 1);
    memory.put_at_rva(PARTNER_VARIANT_POINTER_RVA, &VARIANT.to_le_bytes());
    let found = read_companions(&memory, |_| None);
    let ids: Vec<EnemyId> = found.iter().map(|companion| companion.id).collect();
    assert_eq!(ids, vec![EnemyId::companion(1), EnemyId::companion(2)]);
    assert!(!found[0].script_mode);
    assert!(found[1].script_mode);
}

#[test]
fn a_destroyed_companion_is_not_read() {
    let mut memory = FakeMemory::new();
    let actor = kaine(&memory);
    put_partner(&mut memory, actor, 9, true, 0);
    memory.put(actor + ACTOR_LIFE_STATE_OFFSET, &5_i32.to_le_bytes());
    assert!(read_companions(&memory, |_| Some((1, 1))).is_empty());
}

#[test]
fn a_companion_whose_life_cannot_be_read_is_sent_with_no_life() {
    let mut memory = FakeMemory::new();
    let actor = kaine(&memory);
    put_partner(&mut memory, actor, 9, true, 0);
    let found = read_companions(&memory, |_| None);
    assert_eq!((found[0].sighting.hp, found[0].sighting.max_hp), (0, 0));
    let state = enemy_state(found[0].id, &found[0].sighting).expect("a usable place");
    assert_eq!((state.hp, state.max_hp), (0, 0));
}

#[test]
fn a_companions_wire_state_is_a_valid_enemy_state() {
    let mut memory = FakeMemory::new();
    let actor = kaine(&memory);
    put_partner(&mut memory, actor, 9, true, 0);
    let found = read_companions(&memory, |_| Some((80, 100)));
    let state = enemy_state(found[0].id, &found[0].sighting).expect("a usable place");
    assert!(state.id.is_companion());
    let snapshot = replicant_proto::EnemySnapshot {
        tick: 1,
        phase: 3,
        part: 0,
        parts: 1,
        total: 1,
        round: 1,
        enemies: vec![state],
    };
    assert!(encode(&Message::EnemySnapshot(snapshot)).is_ok());
}

#[test]
fn the_guest_holds_a_companion_the_host_sends_with_a_motion() {
    assert_eq!(companion_control(FOLLOWING), CompanionControl::Host);
}

#[test]
fn a_scene_on_this_machine_owns_the_companion_before_anything_else() {
    let inputs = CompanionGuestInputs {
        in_event_scene: true,
        script_mode: true,
        ..FOLLOWING
    };
    assert_eq!(
        companion_control(inputs),
        CompanionControl::OwnLogic(CompanionOwnLogicCause::EventScene)
    );
}

#[test]
fn a_script_driving_the_companion_here_is_never_held() {
    let inputs = CompanionGuestInputs {
        script_mode: true,
        ..FOLLOWING
    };
    assert_eq!(
        companion_control(inputs),
        CompanionControl::OwnLogic(CompanionOwnLogicCause::ScriptMode)
    );
}

#[test]
fn a_companion_the_host_does_not_send_runs_on_its_own() {
    let inputs = CompanionGuestInputs {
        host_motion_known: None,
        ..FOLLOWING
    };
    assert_eq!(
        companion_control(inputs),
        CompanionControl::OwnLogic(CompanionOwnLogicCause::HostDoesNotSendIt)
    );
}

#[test]
fn a_companion_is_not_held_with_snapping_off_or_without_the_hosts_motion() {
    let snap_off = CompanionGuestInputs {
        snap_enabled: false,
        ..FOLLOWING
    };
    assert_eq!(
        companion_control(snap_off),
        CompanionControl::OwnLogic(CompanionOwnLogicCause::SnapOff)
    );
    let no_motion = CompanionGuestInputs {
        host_motion_known: Some(false),
        ..FOLLOWING
    };
    assert_eq!(
        companion_control(no_motion),
        CompanionControl::OwnLogic(CompanionOwnLogicCause::HostMotionUnknown)
    );
}

#[test]
fn the_host_sends_no_companion_in_a_scene_or_right_after_one() {
    assert!(host_sends_companion(false, false));
    assert!(!host_sends_companion(true, false));
    assert!(!host_sends_companion(false, true));
}

#[test]
fn a_guest_hit_on_a_companion_counts_only_when_its_life_fell_here() {
    assert!(companion_hit_counts(Some(100), Some(90)));
    assert!(!companion_hit_counts(Some(100), Some(100)));
    assert!(!companion_hit_counts(Some(90), Some(100)));
    assert!(!companion_hit_counts(None, Some(90)));
    assert!(!companion_hit_counts(Some(100), None));
}

#[test]
fn only_a_change_of_control_is_reported() {
    let mut log = CompanionControlLog::default();
    let id = EnemyId::companion(0);
    let held = [(id, CompanionControl::Host)];
    assert_eq!(log.observe(&held), held.to_vec());
    assert!(log.observe(&held).is_empty());
    let own = [(
        id,
        CompanionControl::OwnLogic(CompanionOwnLogicCause::EventScene),
    )];
    assert_eq!(log.observe(&own), own.to_vec());
    assert!(log.observe(&[]).is_empty());
    assert_eq!(log.observe(&held), held.to_vec());
    log.reset();
    assert_eq!(log.observe(&held), held.to_vec());
}

#[test]
fn the_control_text_names_the_companion_and_the_cause() {
    let id = EnemyId::companion(0);
    assert_eq!(companion_name(id), "unique actor -2");
    assert_eq!(companion_name(EnemyId::companion(1)), "the third member");
    assert_eq!(
        companion_name(EnemyId {
            group: 3,
            ordinal: 0
        }),
        "an unknown companion"
    );
    assert_eq!(
        companion_control_text(id, CompanionControl::Host),
        "unique actor -2 is held here and follows the host's (place, motion and life)"
    );
    assert_eq!(
        companion_control_text(
            id,
            CompanionControl::OwnLogic(CompanionOwnLogicCause::ScriptMode)
        ),
        "unique actor -2 is left to this game: a script drives it here"
    );
}
