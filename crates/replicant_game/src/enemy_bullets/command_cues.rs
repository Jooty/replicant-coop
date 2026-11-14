use replicant_proto::ShotCueKind;

use crate::enemy_bullets::{
    SCRIPT_BULLET_ATTACH_EFFECT_COMMAND, SCRIPT_BULLET_END_ATTACHED_EFFECT_COMMAND,
    SCRIPT_BULLET_LASTING_SOUND_COMMANDS, SCRIPT_BULLET_LOOK_COMMANDS,
    SCRIPT_BULLET_STOP_LASTING_SOUND_COMMANDS,
};

/// What an effect made by id inside bullet script `command` is to the copy: `None` for the
/// look-slot commands, whose effect already crosses as the shot's look.
pub fn effect_cue_of_command(command: i32) -> Option<ShotCueKind> {
    if SCRIPT_BULLET_LOOK_COMMANDS.contains(&command) {
        return None;
    }
    Some(if command == SCRIPT_BULLET_ATTACH_EFFECT_COMMAND {
        ShotCueKind::FollowingEffect
    } else {
        ShotCueKind::OneShotEffect
    })
}

/// What a sound made by id inside bullet script `command` is to the copy.
pub fn sound_cue_of_command(command: i32) -> ShotCueKind {
    if SCRIPT_BULLET_LASTING_SOUND_COMMANDS.contains(&command) {
        ShotCueKind::LastingSound
    } else {
        ShotCueKind::OneShotSound
    }
}

/// The end a bullet script `command` makes of what the shot made before, if any.
pub fn end_cue_of_command(command: i32) -> Option<ShotCueKind> {
    if SCRIPT_BULLET_STOP_LASTING_SOUND_COMMANDS.contains(&command) {
        Some(ShotCueKind::EndLastingSound)
    } else if command == SCRIPT_BULLET_END_ATTACHED_EFFECT_COMMAND {
        Some(ShotCueKind::EndFollowingEffects)
    } else {
        None
    }
}
