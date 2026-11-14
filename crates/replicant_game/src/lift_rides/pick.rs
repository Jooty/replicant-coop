use crate::lift_rides::{LIFT_ORIGIN_POINT_CHAINS, LIFT_PHASES};

/// A floor pick on a lift panel: `origin` is the warp point the panel opened on, `point` the
/// floor picked, `chain` the script function the game's `SelectWarpPoint` starts for the pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiftRide {
    pub origin: u16,
    pub point: u16,
    pub chain: &'static str,
}

/// The lift ride a warp pick in `phase` starts, or `None` for any other travel menu, a cancel,
/// or a pair the game's `SelectWarpPoint` has no lift branch for.
pub fn lift_ride(phase: &str, point: i32, origin: i32) -> Option<LiftRide> {
    if !LIFT_PHASES.contains(&phase) {
        return None;
    }
    let point = u16::try_from(point).ok()?;
    let origin = u16::try_from(origin).ok()?;
    LIFT_ORIGIN_POINT_CHAINS
        .iter()
        .find(|(from, to, _)| *from == origin && *to == point)
        .map(|(_, _, chain)| LiftRide {
            origin,
            point,
            chain,
        })
}

/// The origin the game queues with a pick: the panel's pick passes -1, and the game then uses
/// the point the warp window was opened on.
pub fn warp_pick_origin(passed_other: i32, window_origin: i32) -> i32 {
    if passed_other >= 0 {
        passed_other
    } else {
        window_origin
    }
}
