use crate::fight_sync::ScriptedCameraNative;
use crate::memory::GameMemory;

pub const FREE_CAMERA_DIRECTION_CONTROL_RVA: u32 = 0x512_5C80;
pub const FREE_CAMERA_DIRECTION_SLOT_COUNT: usize = 9;
pub const FREE_CAMERA_DIRECTION_SLOT_STRIDE_BYTES: u64 = 0x2B0;
pub const FREE_CAMERA_DIRECTION_SLOT_PITCH_LERP_OFFSET: u64 = 0x158;
pub const FREE_CAMERA_BASE_YAW_OFFSET: u64 = 0x1850;
pub const FREE_CAMERA_BASE_PITCH_OFFSET: u64 = 0x18F0;
pub const SCRIPTED_FREE_DIRECTION_SLOT: usize = 7;
pub const CAMERA_SWITCH_LOCK_BYTE_RVA: u32 = 0x32E_7EC8;
pub const CAMERA_SWITCH_BLOCK_BYTE_RVA: u32 = 0x32E_79D2;
pub const CAMERA_SWITCH_FORCES_SNOW_BYTE_RVA: u32 = 0x32E_79D1;
pub const CAMERA_SWITCH_ONLY_CAMERA_2_BYTE_RVA: u32 = 0x38E_2E00;
pub const CAMERA_SWITCH_ONLY_CAMERA_2_OVERRIDE_BYTE_RVA: u32 = 0x32C_6164;
pub const SNOW_CAMERA_INDEX: i32 = 0;
pub const FIXED_CAMERA_INDEX: i32 = 8;
pub const DIRECTION_SET_MATCH_RADIANS: f32 = 0.01;
pub const REFUSED_CAMERA_CALL_RETRY_EVERY_MS: u64 = 250;
pub const REFUSED_CAMERA_CALL_GIVE_UP_MS: u64 = 10_000;
pub const CAMERA_FOLLOW_UP_LOG_MIN_MS: u64 = 1_000;
pub const CAMERA_STATE_LOG_EVERY_MS: u64 = 1_000;

/// The free camera's direction control inside `CSnowCamera`: the base yaw and pitch it turns
/// from (radians; positive pitch looks down) and which of its nine request slots are turning.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FreeDirectionState {
    pub base_yaw: f32,
    pub base_pitch: f32,
    pub turning_slots: u16,
}

/// The bytes `CameraManager::SwitchTo` checks before it changes the active camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CameraSwitchGates {
    pub manager_lock: bool,
    pub switch_block: bool,
    pub forces_snow: bool,
    pub only_camera_2: bool,
    pub only_camera_2_override: bool,
}

/// Whether a scripted camera call did what it asked, judged right after it returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraCallOutcome {
    Took,
    Refused,
    NotJudged,
}

/// A refused call of the local script, kept to run again with its own numbers.
#[derive(Debug, Clone, PartialEq)]
pub struct RefusedCameraCall {
    pub native: ScriptedCameraNative,
    pub numbers: Vec<Option<f64>>,
    pub phase: u16,
    pub refused_at_ms: u64,
    pub last_try_ms: u64,
    pub tries: u32,
}

/// The guest's refused calls; only the newest one is kept, since a later call replaces it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RefusedCameraCalls {
    pending: Option<RefusedCameraCall>,
}

/// What the retry poll asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum RefusedCallStep {
    RunAgain(RefusedCameraCall),
    GaveUp(RefusedCameraCall),
}

impl FreeDirectionState {
    pub fn slot_is_turning(self, slot: usize) -> bool {
        slot < FREE_CAMERA_DIRECTION_SLOT_COUNT && self.turning_slots & (1 << slot) != 0
    }
}

impl CameraSwitchGates {
    /// Whether a switch to camera `target` would be refused now (`SwitchTo` 0x343330).
    pub fn refuses_switch_to(self, target_is_camera_2: bool) -> bool {
        self.manager_lock
            || self.switch_block
            || (self.only_camera_2 && !self.only_camera_2_override && !target_is_camera_2)
    }
}

impl RefusedCameraCalls {
    pub fn note_outcome(
        &mut self,
        native: ScriptedCameraNative,
        numbers: &[Option<f64>],
        phase: u16,
        outcome: CameraCallOutcome,
        now_ms: u64,
    ) {
        match outcome {
            CameraCallOutcome::Refused if runs_again_when_refused(native) => {
                let same_call = self
                    .pending
                    .as_ref()
                    .is_some_and(|call| call.native == native && call.numbers == numbers);
                if same_call {
                    return;
                }
                self.pending = Some(RefusedCameraCall {
                    native,
                    numbers: numbers.to_vec(),
                    phase,
                    refused_at_ms: now_ms,
                    last_try_ms: now_ms,
                    tries: 0,
                });
            }
            CameraCallOutcome::Refused | CameraCallOutcome::Took | CameraCallOutcome::NotJudged => {
                if self
                    .pending
                    .as_ref()
                    .is_some_and(|call| call.native == native)
                {
                    self.pending = None;
                }
            }
        }
    }

    pub fn poll(&mut self, now_ms: u64, phase: Option<u16>) -> Option<RefusedCallStep> {
        let call = self.pending.as_mut()?;
        if phase != Some(call.phase)
            || now_ms.saturating_sub(call.refused_at_ms) >= REFUSED_CAMERA_CALL_GIVE_UP_MS
        {
            return self.pending.take().map(RefusedCallStep::GaveUp);
        }
        if now_ms.saturating_sub(call.last_try_ms) < REFUSED_CAMERA_CALL_RETRY_EVERY_MS {
            return None;
        }
        call.last_try_ms = now_ms;
        call.tries += 1;
        Some(RefusedCallStep::RunAgain(call.clone()))
    }

    pub fn pending(&self) -> Option<&RefusedCameraCall> {
        self.pending.as_ref()
    }
}

pub fn read_free_direction_state<M: GameMemory>(memory: &M) -> Option<FreeDirectionState> {
    let control = memory.module_base() + u64::from(FREE_CAMERA_DIRECTION_CONTROL_RVA);
    let mut turning_slots = 0_u16;
    for slot in 0..FREE_CAMERA_DIRECTION_SLOT_COUNT {
        let at = control + slot as u64 * FREE_CAMERA_DIRECTION_SLOT_STRIDE_BYTES;
        let yaw_turning = memory.read_u8(at).ok()? != 0;
        let pitch_turning = memory
            .read_u8(at + FREE_CAMERA_DIRECTION_SLOT_PITCH_LERP_OFFSET)
            .ok()?
            != 0;
        if yaw_turning || pitch_turning {
            turning_slots |= 1 << slot;
        }
    }
    Some(FreeDirectionState {
        base_yaw: memory
            .read_f32(control + FREE_CAMERA_BASE_YAW_OFFSET)
            .ok()?,
        base_pitch: memory
            .read_f32(control + FREE_CAMERA_BASE_PITCH_OFFSET)
            .ok()?,
        turning_slots,
    })
}

pub fn read_camera_switch_gates<M: GameMemory>(memory: &M) -> Option<CameraSwitchGates> {
    let byte = |rva: u32| {
        memory
            .read_u8(memory.module_base() + u64::from(rva))
            .ok()
            .map(|value| value != 0)
    };
    Some(CameraSwitchGates {
        manager_lock: byte(CAMERA_SWITCH_LOCK_BYTE_RVA)?,
        switch_block: byte(CAMERA_SWITCH_BLOCK_BYTE_RVA)?,
        forces_snow: byte(CAMERA_SWITCH_FORCES_SNOW_BYTE_RVA)?,
        only_camera_2: byte(CAMERA_SWITCH_ONLY_CAMERA_2_BYTE_RVA)?,
        only_camera_2_override: byte(CAMERA_SWITCH_ONLY_CAMERA_2_OVERRIDE_BYTE_RVA)?,
    })
}

/// The yaw and pitch (radians) `_SetFreeCameraDirectionFixed` asks for: from its first point
/// to its second, as 0x6DBD30 and 0x6DB8C0 compute them.
pub fn requested_free_direction(numbers: &[Option<f64>]) -> Option<(f32, f32)> {
    let point = |first: usize| -> Option<[f32; 3]> {
        Some([
            (*numbers.get(first)?)? as f32,
            (*numbers.get(first + 1)?)? as f32,
            (*numbers.get(first + 2)?)? as f32,
        ])
    };
    let (from, to) = (point(0)?, point(3)?);
    let [dx, dy, dz] = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    let flat = dx.hypot(dz);
    let yaw = if flat > 0.0 { dx.atan2(dz) } else { 0.0 };
    Some((yaw, (-dy).atan2(flat)))
}

pub fn requested_blend_seconds(native: ScriptedCameraNative, numbers: &[Option<f64>]) -> f64 {
    let at = match native {
        ScriptedCameraNative::SetFreeCameraDirectionFixed => 7,
        ScriptedCameraNative::ChangeCamera => 1,
        ScriptedCameraNative::ResetFreeCameraDirectionWithActorGroup => 3,
        ScriptedCameraNative::SetFixedCamera
        | ScriptedCameraNative::SetFreeCameraDistanceOffset => return 0.0,
    };
    numbers.get(at).copied().flatten().unwrap_or(0.0)
}

/// The camera index `_ChangeCamera`'s first argument picks, where it is a fixed one.
pub fn camera_index_of_change_kind(kind: f64) -> Option<i32> {
    match kind as i32 {
        0 => Some(SNOW_CAMERA_INDEX),
        1 => Some(FIXED_CAMERA_INDEX),
        _ => None,
    }
}

/// Judges a call: a direction request is refused when a slot above the script's was turning
/// before it (`ClaimSlot` 0x125260); a camera change when the camera it names is not active
/// after it.
pub fn judge_camera_call(
    native: ScriptedCameraNative,
    numbers: &[Option<f64>],
    free_before: Option<FreeDirectionState>,
    active_camera_after: Option<i32>,
) -> CameraCallOutcome {
    match native {
        ScriptedCameraNative::SetFreeCameraDirectionFixed => {
            let Some(before) = free_before else {
                return CameraCallOutcome::NotJudged;
            };
            let higher_slot_turning = (SCRIPTED_FREE_DIRECTION_SLOT + 1
                ..FREE_CAMERA_DIRECTION_SLOT_COUNT)
                .any(|slot| before.slot_is_turning(slot));
            if higher_slot_turning {
                CameraCallOutcome::Refused
            } else {
                CameraCallOutcome::Took
            }
        }
        ScriptedCameraNative::ChangeCamera => {
            let target = numbers
                .first()
                .copied()
                .flatten()
                .and_then(camera_index_of_change_kind);
            match (target, active_camera_after) {
                (Some(target), Some(active)) if target == active => CameraCallOutcome::Took,
                (Some(_), Some(_)) => CameraCallOutcome::Refused,
                _ => CameraCallOutcome::NotJudged,
            }
        }
        ScriptedCameraNative::SetFixedCamera
        | ScriptedCameraNative::SetFreeCameraDistanceOffset
        | ScriptedCameraNative::ResetFreeCameraDirectionWithActorGroup => {
            CameraCallOutcome::NotJudged
        }
    }
}

/// Whether the free direction now holds what a zero-time `_SetFreeCameraDirectionFixed` asked
/// for; `None` when the call blends or is unreadable.
pub fn free_direction_matches_request(
    numbers: &[Option<f64>],
    free_after: Option<FreeDirectionState>,
) -> Option<bool> {
    if requested_blend_seconds(ScriptedCameraNative::SetFreeCameraDirectionFixed, numbers) > 0.0 {
        return None;
    }
    let after = free_after?;
    let (yaw, pitch) = requested_free_direction(numbers)?;
    Some(
        angle_gap_radians(after.base_yaw, yaw) <= DIRECTION_SET_MATCH_RADIANS
            && angle_gap_radians(after.base_pitch, pitch) <= DIRECTION_SET_MATCH_RADIANS,
    )
}

/// Only a refused direction request is run again: a refused camera change can be the game's
/// own order during a scene, which the host's game keeps too.
pub fn runs_again_when_refused(native: ScriptedCameraNative) -> bool {
    native == ScriptedCameraNative::SetFreeCameraDirectionFixed
}

/// Which of a native's arguments are integers; the rest are reals.
pub fn integer_argument_slots(native: ScriptedCameraNative) -> &'static [usize] {
    match native {
        ScriptedCameraNative::SetFreeCameraDirectionFixed => &[6],
        ScriptedCameraNative::ChangeCamera => &[0, 2],
        ScriptedCameraNative::ResetFreeCameraDirectionWithActorGroup => &[0, 1],
        ScriptedCameraNative::SetFixedCamera
        | ScriptedCameraNative::SetFreeCameraDistanceOffset => &[],
    }
}

pub fn angle_gap_radians(a: f32, b: f32) -> f32 {
    let gap = (a - b).rem_euclid(core::f32::consts::TAU);
    gap.min(core::f32::consts::TAU - gap)
}

pub fn free_direction_text(state: Option<FreeDirectionState>) -> String {
    state.map_or_else(
        || "free direction unreadable".to_owned(),
        |state| {
            let slots: Vec<String> = (0..FREE_CAMERA_DIRECTION_SLOT_COUNT)
                .filter(|slot| state.slot_is_turning(*slot))
                .map(|slot| slot.to_string())
                .collect();
            format!(
                "free direction yaw {:.1} pitch {:.1} (down positive), slots turning [{}]",
                state.base_yaw.to_degrees(),
                state.base_pitch.to_degrees(),
                slots.join(",")
            )
        },
    )
}

pub fn switch_gates_text(gates: Option<CameraSwitchGates>) -> String {
    gates.map_or_else(
        || "switch gates unreadable".to_owned(),
        |gates| {
            format!(
                "switch gates lock {} block {} forces-snow {} only-camera-2 {} (override {})",
                u8::from(gates.manager_lock),
                u8::from(gates.switch_block),
                u8::from(gates.forces_snow),
                u8::from(gates.only_camera_2),
                u8::from(gates.only_camera_2_override)
            )
        },
    )
}

pub fn camera_outcome_text(outcome: CameraCallOutcome) -> &'static str {
    match outcome {
        CameraCallOutcome::Took => "it took",
        CameraCallOutcome::Refused => "REFUSED by the game",
        CameraCallOutcome::NotJudged => "not judged",
    }
}

/// One camera state line: `what` says when it was read (`after _ChangeCamera(0,0,2)`).
pub fn camera_state_line(
    what: &str,
    active_camera: Option<i32>,
    forward: Option<[f32; 3]>,
    free: Option<FreeDirectionState>,
    gates: Option<CameraSwitchGates>,
) -> String {
    format!(
        "scripted camera: {what}: camera {} active, it looks {}; {}; {}",
        active_camera.map_or_else(|| "?".to_owned(), |index| index.to_string()),
        crate::fight_sync::direction_text(forward),
        free_direction_text(free),
        switch_gates_text(gates)
    )
}
