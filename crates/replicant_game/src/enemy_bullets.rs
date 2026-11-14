//! Bullets the host's enemies fire, scanned on the host and drawn on the guest as effects or
//! through the game's sprite batch.

mod board;
mod command_cues;
mod copy_motion;
mod copy_motion_tally;
mod first_sightings;
mod kinds;
mod laser_beam;
mod look_scale;
mod owner;
mod puppet_pass;
mod refusals;
mod scale_growth;
mod scan;
mod script_bullet;
mod sender;
mod shot_cue_gate;
mod shot_cue_tally;
mod shot_instances;
mod shown_effects;
mod sprite_frames;
mod uncopied;

pub use board::{EnemyBulletBoard, EnemyBulletStep};
pub use command_cues::{effect_cue_of_command, end_cue_of_command, sound_cue_of_command};
pub use copy_motion::{CopyCorrection, CopyMotion};
pub use copy_motion_tally::CopyMotionTally;
pub use first_sightings::BulletFirstSightings;
pub use kinds::{EnemyBulletKinds, SpriteBulletKinds};
pub use laser_beam::{
    BeamEnd, LaserBeam, LaserBeamTally, beam_end_text, longest_axis, read_laser_beam,
};
pub use look_scale::{look_resized, look_scale, scaled_look};
pub use owner::bullet_owner;
pub use puppet_pass::{
    PuppetPassInputs, bullet_reaches_puppet, needs_puppet_pass, script_bullet_deals_damage,
};
pub use refusals::FlightRefusals;
pub use scale_growth::{scale_growth_of_radius, shot_scale_growth};
pub use scan::{
    EnemyBullet, EnemyBulletCensus, EnemyBulletScan, UncopiedScriptBullet, bullets_of_enemies,
    scan_enemy_bullets,
};
pub use script_bullet::{
    NotCopyable, UncopiedWhy, read_script_bullet, script_bullet_is_laser, why_not_copied,
};
pub use sender::EnemyBulletSender;
pub use shot_cue_gate::{CueAdmission, ShotCueGate};
pub use shot_cue_tally::ShotCueTally;
pub use shot_instances::ShotInstances;
pub use shown_effects::ShownEffectTally;
pub use sprite_frames::{first_sprite_frame, next_sprite_frame};
pub use uncopied::{UncopiedTally, uncopied_why_text};

pub const ENEMY_BULLET_SCAN_INTERVAL_MS: u64 = 50;
pub const ENEMY_BULLET_STALE_MS: u64 = 600;
pub const MAX_SIGHTED_ENEMY_BULLETS: usize = 48;
pub const MAX_SIGHTED_ENEMY_SPRITE_BULLETS: usize = 80;
pub const MAX_SHOWN_ENEMY_BULLETS: usize = 128;
pub const MAX_ENEMY_BULLET_MOVES_SENT_PER_SCAN: usize = 24;
pub const MAX_ENEMY_BULLET_KINDS_LOGGED: usize = 64;
pub const MAX_SPRITE_BULLET_KINDS_LOGGED: usize = 16;
pub const BULLET_SPRITE_FRAME_COUNT: u16 = 64;
pub const MAX_UNCOPIED_SCRIPT_BULLETS_READ: usize = 32;
pub const SCRIPT_BULLET_LASER_FLAG_OFFSET: u64 = 0x1698;
pub const SCRIPT_BULLET_EXECUTE_COMMAND_RVA: u32 = 0x33_0FA0;
pub const SCRIPT_BULLET_COMMAND_RECEIVER_OFFSET: u64 = 0x1900;
pub const SCRIPT_BULLET_LOOK_COMMANDS: [i32; 2] = [0x39, 0x3B];
pub const SCRIPT_BULLET_ATTACH_EFFECT_COMMAND: i32 = 0x3D;
pub const SCRIPT_BULLET_END_ATTACHED_EFFECT_COMMAND: i32 = 0x3E;
pub const SCRIPT_BULLET_LASTING_SOUND_COMMANDS: [i32; 2] = [0x43, 0x45];
pub const SCRIPT_BULLET_STOP_LASTING_SOUND_COMMANDS: [i32; 2] = [0x44, 0x46];
pub const SHOT_CUE_REPEAT_GAP_MS: u64 = 50;
pub const MAX_SHOT_CUE_REPEATS_REMEMBERED: usize = 512;
pub const SHOT_CUE_SUMMARY_INTERVAL_MS: u64 = 1000;
pub const MAX_FOLLOWING_EFFECTS_PER_COPY: usize = 4;
pub const REPEAT_WITHIN_THE_GAP: &str = "a repeat within 50 ms";
pub const MAX_LASER_BEAM_KINDS_NAMED: usize = 16;
pub const MAX_SHOWN_BULLET_EFFECTS_LOGGED: usize = 8;
pub const PUPPET_BULLET_REACH_MARGIN: f32 = 4.0;
pub const MAX_BULLETS_REMEMBERED_FOR_LOG: usize = 64;
pub const SHOT_CORRECTION_BLEND_MS: u64 = 100;
pub const MAX_SMOOTHED_SHOT_PLACE_CORRECTION_METRES: f32 = 4.0;
pub const MAX_SMOOTHED_SHOT_SCALE_CORRECTION_RATIO: f32 = 4.0;
