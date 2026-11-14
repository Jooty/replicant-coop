use replicant_game::fight_sync::{
    CameraCallOutcome, CameraSwitchGates, FREE_CAMERA_BASE_PITCH_OFFSET,
    FREE_CAMERA_BASE_YAW_OFFSET, FREE_CAMERA_DIRECTION_CONTROL_RVA,
    FREE_CAMERA_DIRECTION_SLOT_COUNT, FREE_CAMERA_DIRECTION_SLOT_PITCH_LERP_OFFSET,
    FREE_CAMERA_DIRECTION_SLOT_STRIDE_BYTES, FreeDirectionState, REFUSED_CAMERA_CALL_GIVE_UP_MS,
    REFUSED_CAMERA_CALL_RETRY_EVERY_MS, RefusedCallStep, RefusedCameraCalls, ScriptedCameraNative,
    free_direction_matches_request, judge_camera_call, read_free_direction_state,
    requested_free_direction,
};

use crate::fake_memory::FakeMemory;

const PHASE: u16 = 16;

fn fight_direction_call() -> Vec<Option<f64>> {
    [
        -11.507, 75.47, 0.0, 61.468, 53.076, 0.0, 0.0, 0.0, 0.1, 1.0, 0.1,
    ]
    .into_iter()
    .map(Some)
    .collect()
}

fn state(turning_slots: u16) -> FreeDirectionState {
    FreeDirectionState {
        base_yaw: core::f32::consts::FRAC_PI_2,
        base_pitch: 0.298,
        turning_slots,
    }
}

#[test]
fn the_fight_direction_looks_along_x_and_down_from_its_first_point_to_its_second() {
    let (yaw, pitch) = requested_free_direction(&fight_direction_call()).unwrap();
    assert!((yaw.to_degrees() - 90.0).abs() < 0.01);
    assert!((pitch.to_degrees() - 17.06).abs() < 0.05);
}

#[test]
fn a_direction_request_is_refused_only_while_a_higher_slot_turns() {
    let call = fight_direction_call();
    let native = ScriptedCameraNative::SetFreeCameraDirectionFixed;
    assert_eq!(
        judge_camera_call(native, &call, Some(state(1 << 8)), Some(0)),
        CameraCallOutcome::Refused
    );
    assert_eq!(
        judge_camera_call(native, &call, Some(state((1 << 7) | (1 << 2))), Some(0)),
        CameraCallOutcome::Took
    );
    assert_eq!(
        judge_camera_call(native, &call, None, Some(0)),
        CameraCallOutcome::NotJudged
    );
}

#[test]
fn a_camera_change_took_when_the_camera_it_names_is_active() {
    let native = ScriptedCameraNative::ChangeCamera;
    let to_free = [Some(0.0), Some(0.0), Some(2.0)];
    assert_eq!(
        judge_camera_call(native, &to_free, None, Some(0)),
        CameraCallOutcome::Took
    );
    assert_eq!(
        judge_camera_call(native, &to_free, None, Some(8)),
        CameraCallOutcome::Refused
    );
    let to_fixed = [Some(1.0), Some(10.0), Some(1.0)];
    assert_eq!(
        judge_camera_call(native, &to_fixed, None, Some(8)),
        CameraCallOutcome::Took
    );
    let to_look_at_enemy = [Some(4.0), Some(0.0), Some(0.0)];
    assert_eq!(
        judge_camera_call(native, &to_look_at_enemy, None, Some(0)),
        CameraCallOutcome::NotJudged
    );
}

#[test]
fn a_zero_time_request_is_held_when_the_base_direction_matches_it() {
    let call = fight_direction_call();
    assert_eq!(
        free_direction_matches_request(&call, Some(state(0))),
        Some(true)
    );
    let turned = FreeDirectionState {
        base_yaw: -1.0,
        ..state(0)
    };
    assert_eq!(
        free_direction_matches_request(&call, Some(turned)),
        Some(false)
    );
    let mut blended = call.clone();
    blended[7] = Some(1.5);
    assert_eq!(
        free_direction_matches_request(&blended, Some(state(0))),
        None
    );
}

#[test]
fn a_refused_direction_request_runs_again_until_it_takes() {
    let native = ScriptedCameraNative::SetFreeCameraDirectionFixed;
    let call = fight_direction_call();
    let mut calls = RefusedCameraCalls::default();
    calls.note_outcome(native, &call, PHASE, CameraCallOutcome::Refused, 1_000);
    assert_eq!(calls.poll(1_100, Some(PHASE)), None);
    let Some(RefusedCallStep::RunAgain(again)) =
        calls.poll(1_000 + REFUSED_CAMERA_CALL_RETRY_EVERY_MS, Some(PHASE))
    else {
        panic!("expected a second run");
    };
    assert_eq!(
        (again.native, again.numbers.clone(), again.tries),
        (native, call.clone(), 1)
    );
    calls.note_outcome(native, &call, PHASE, CameraCallOutcome::Refused, 1_300);
    assert_eq!(calls.pending().unwrap().refused_at_ms, 1_000);
    calls.note_outcome(native, &call, PHASE, CameraCallOutcome::Took, 1_600);
    assert_eq!(calls.pending(), None);
    assert_eq!(calls.poll(2_000, Some(PHASE)), None);
}

#[test]
fn a_refused_request_is_given_up_after_the_limit_or_in_another_phase() {
    let native = ScriptedCameraNative::SetFreeCameraDirectionFixed;
    let call = fight_direction_call();
    let mut calls = RefusedCameraCalls::default();
    calls.note_outcome(native, &call, PHASE, CameraCallOutcome::Refused, 0);
    assert!(matches!(
        calls.poll(REFUSED_CAMERA_CALL_GIVE_UP_MS, Some(PHASE)),
        Some(RefusedCallStep::GaveUp(_))
    ));
    calls.note_outcome(native, &call, PHASE, CameraCallOutcome::Refused, 0);
    assert!(matches!(
        calls.poll(300, Some(PHASE + 1)),
        Some(RefusedCallStep::GaveUp(_))
    ));
    assert_eq!(calls.pending(), None);
}

#[test]
fn a_refused_camera_change_is_not_run_again() {
    let mut calls = RefusedCameraCalls::default();
    calls.note_outcome(
        ScriptedCameraNative::ChangeCamera,
        &[Some(0.0), Some(0.0), Some(2.0)],
        PHASE,
        CameraCallOutcome::Refused,
        0,
    );
    assert_eq!(calls.pending(), None);
}

#[test]
fn the_switch_gates_refuse_as_switch_to_does() {
    let open = CameraSwitchGates::default();
    assert!(!open.refuses_switch_to(false));
    let only_camera_2 = CameraSwitchGates {
        only_camera_2: true,
        ..open
    };
    assert!(only_camera_2.refuses_switch_to(false));
    assert!(!only_camera_2.refuses_switch_to(true));
    let overridden = CameraSwitchGates {
        only_camera_2_override: true,
        ..only_camera_2
    };
    assert!(!overridden.refuses_switch_to(false));
    let locked = CameraSwitchGates {
        manager_lock: true,
        ..open
    };
    assert!(locked.refuses_switch_to(true));
}

#[test]
fn the_free_direction_state_is_read_from_the_snow_cameras_control() {
    let mut memory = FakeMemory::new();
    let control = u64::from(FREE_CAMERA_DIRECTION_CONTROL_RVA);
    for slot in 0..FREE_CAMERA_DIRECTION_SLOT_COUNT as u64 {
        let at = control + slot * FREE_CAMERA_DIRECTION_SLOT_STRIDE_BYTES;
        let yaw_turning = u8::from(slot == 7);
        let pitch_turning = u8::from(slot == 8);
        memory.put_at_rva(at as u32, &[yaw_turning]);
        memory.put_at_rva(
            (at + FREE_CAMERA_DIRECTION_SLOT_PITCH_LERP_OFFSET) as u32,
            &[pitch_turning],
        );
    }
    memory.put_at_rva(
        (control + FREE_CAMERA_BASE_YAW_OFFSET) as u32,
        &1.5_f32.to_le_bytes(),
    );
    memory.put_at_rva(
        (control + FREE_CAMERA_BASE_PITCH_OFFSET) as u32,
        &0.25_f32.to_le_bytes(),
    );
    let read = read_free_direction_state(&memory).unwrap();
    assert_eq!(read.turning_slots, (1 << 7) | (1 << 8));
    assert!(read.slot_is_turning(8) && !read.slot_is_turning(0));
    assert_eq!((read.base_yaw, read.base_pitch), (1.5, 0.25));
}
