use replicant_game::story_jump::SET_PLAYER_POSITION_NATIVE_RVA;
use replicant_game::{Position, Replicant};

use crate::build_gate::VerifiedBuild;
use crate::memory::InProcessMemory;
use crate::story_jump::{NativeArg, call_native_with};
use crate::teleport::verify;

pub(crate) fn move_player_with_native(
    build: &VerifiedBuild,
    game: &Replicant<InProcessMemory>,
    spot: Position,
) -> f32 {
    let heading_degrees = game.yaw_radians().map_or(0.0, f32::to_degrees);
    call_native_with(
        build,
        SET_PLAYER_POSITION_NATIVE_RVA,
        &[
            NativeArg::Real(f64::from(spot.x)),
            NativeArg::Real(f64::from(spot.y)),
            NativeArg::Real(f64::from(spot.z)),
            NativeArg::Real(f64::from(heading_degrees)),
        ],
    );
    verify::expect_arrival([spot.x, spot.y, spot.z]);
    heading_degrees
}

pub(crate) fn move_player_facing(build: &VerifiedBuild, spot: Position, heading_degrees: f32) {
    call_native_with(
        build,
        SET_PLAYER_POSITION_NATIVE_RVA,
        &[
            NativeArg::Real(f64::from(spot.x)),
            NativeArg::Real(f64::from(spot.y)),
            NativeArg::Real(f64::from(spot.z)),
            NativeArg::Real(f64::from(heading_degrees)),
        ],
    );
    verify::expect_arrival([spot.x, spot.y, spot.z]);
}
