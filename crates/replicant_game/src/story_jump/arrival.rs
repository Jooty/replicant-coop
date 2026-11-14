use replicant_proto::StoryJumpPoint;

use crate::story_jump::LEVEL_CHECK_SETTLE_MS;

const ARENA_HOST_POSITION: [f32; 3] = [-2.2, 146.8, -8.6];
const ARENA_GUEST_SIDE_OFFSET_UNITS: f32 = 2.0;
const JUNK_HEAP_FLOOR_B2_HOST_POSITION: [f32; 3] = [-76.5, 4.9, -124.0];
const JUNK_HEAP_FLOOR_B2_GUEST_X_OFFSET_UNITS: f32 = -0.6;
const AFTER_BOSS_FIGHT_6_HOST_POSITION: [f32; 3] = [26.25, 30.61, 107.67];
const AFTER_BOSS_FIGHT_6_GUEST_X_OFFSET_UNITS: f32 = 1.5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArrivalSpot {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub fn arrival_spot(point: StoryJumpPoint, host: bool) -> Option<ArrivalSpot> {
    match point {
        StoryJumpPoint::AtShrineBoss => {
            let [x, y, z] = ARENA_HOST_POSITION;
            let side = if host {
                0.0
            } else {
                ARENA_GUEST_SIDE_OFFSET_UNITS
            };
            Some(ArrivalSpot { x: x + side, y, z })
        }
        StoryJumpPoint::JunkHeapFloorB2 => {
            let [x, y, z] = JUNK_HEAP_FLOOR_B2_HOST_POSITION;
            let side = if host {
                0.0
            } else {
                JUNK_HEAP_FLOOR_B2_GUEST_X_OFFSET_UNITS
            };
            Some(ArrivalSpot { x: x + side, y, z })
        }
        StoryJumpPoint::AfterBossFight6 => {
            let [x, y, z] = AFTER_BOSS_FIGHT_6_HOST_POSITION;
            let side = if host {
                0.0
            } else {
                AFTER_BOSS_FIGHT_6_GUEST_X_OFFSET_UNITS
            };
            Some(ArrivalSpot { x: x + side, y, z })
        }
        _ => None,
    }
}

pub fn arrival_is_due(
    phase_at_apply: &str,
    phase_now: &str,
    field_was_left: bool,
    field_ready: bool,
    ms_since_phase_change: u64,
) -> bool {
    let phase_changed = phase_at_apply != phase_now || field_was_left;
    phase_changed && field_ready && ms_since_phase_change >= LEVEL_CHECK_SETTLE_MS
}
