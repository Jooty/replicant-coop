use replicant_proto::EnemyId;

use crate::kaine::GUEST_HIT_SCALE_IN_A_STAND_IN_BEAT;

/// A stretch of a companion-off fight in which the guest stands where the companion would: the
/// host's game flag `opened_by_game_flag` is on and `closed_by_game_flag` is still off.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StandInBeat {
    pub fight: &'static str,
    pub beat: u8,
    pub opened_by_game_flag: u16,
    pub closed_by_game_flag: u16,
    pub spot: [f32; 3],
    pub yaw_degrees: f32,
    /// The guest is moved to the spot once one of these has run on its own game.
    pub move_after: &'static [&'static str],
    /// Trap callbacks the guest's own game must not run before the host's game has.
    pub held_traps: &'static [&'static str],
    /// Held traps whose host copy the story relay runs on the guest's game: the guest's own copy
    /// is never run while the host is there, or the host would be asked to run it again.
    pub held_traps_the_host_runs_here: &'static [&'static str],
    pub on_the_way_game_flags: &'static [u16],
    pub boss: EnemyId,
}

pub const STAND_IN_BEATS: [StandInBeat; 2] = [
    StandInBeat {
        fight: "boss fight 6",
        beat: 1,
        opened_by_game_flag: 378,
        closed_by_game_flag: 379,
        spot: [-0.07, 36.6, -0.56],
        yaw_degrees: 90.0,
        move_after: &[
            "EID_1050_a0030_End",
            "EID_1050_a0035_Start",
            "EID_1050_a0035g_01_Continue",
            "EID_1050_a0035g_02_Continue",
        ],
        held_traps: &["EID_1050_a0035_a0040"],
        held_traps_the_host_runs_here: &["EID_1050_a0035_a0040"],
        on_the_way_game_flags: &[1307],
        boss: EnemyId {
            group: 100,
            ordinal: 0,
        },
    },
    StandInBeat {
        fight: "boss fight 6",
        beat: 2,
        opened_by_game_flag: 380,
        closed_by_game_flag: 383,
        spot: [-42.76, 57.36, -48.3],
        yaw_degrees: 65.5,
        move_after: &[
            "EID_1050_a0040_Move_End",
            "EID_1050_a0045_Start",
            "EID_1050_a0045g_Continue",
            "EID_1050_a0050_Continue",
            "EID_1050_a0050_01_End",
        ],
        held_traps: &[
            "Put_Enemy_AL3_090",
            "FLAG_DELETE_TABLE03",
            "EID_1050_a0055_Start",
        ],
        held_traps_the_host_runs_here: &[],
        on_the_way_game_flags: &[1308, 1309],
        boss: EnemyId {
            group: 100,
            ordinal: 0,
        },
    },
];

/// The beat of `fight` that the host's flags say is open, if any. Read fresh each time, so a
/// retry that reloads the flags rebuilds it.
pub fn open_stand_in_beat(
    fight: &str,
    host_game_flag: impl Fn(u16) -> bool,
) -> Option<&'static StandInBeat> {
    STAND_IN_BEATS.iter().find(|beat| {
        beat.fight == fight
            && host_game_flag(beat.opened_by_game_flag)
            && !host_game_flag(beat.closed_by_game_flag)
    })
}

pub fn stand_in_beat_moved_after(function: &str) -> Option<&'static StandInBeat> {
    STAND_IN_BEATS
        .iter()
        .find(|beat| beat.move_after.contains(&function))
}

/// A guest hit on the beat's boss while the beat is open lands at a fraction of its HP damage,
/// never below 1 for a hit that did any.
pub fn stand_in_hp_damage(hp_damage: u32) -> u32 {
    if hp_damage == 0 {
        return 0;
    }
    let scaled = (f64::from(hp_damage) * f64::from(GUEST_HIT_SCALE_IN_A_STAND_IN_BEAT)).round();
    // the scale is below 1, so the rounded value fits the u32 it came from
    (scaled as u32).max(1)
}
