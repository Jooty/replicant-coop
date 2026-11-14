use nc_hook::stall_watch::in_game_thread_step;

use crate::actions::action::Action;
use crate::actions::phase_change::change_phase;
use crate::actions::queue::take_all;
use crate::actions::trainer::run_trainer_action;
use crate::boss_messages::tick as tick_boss_messages;
use crate::build_gate::VerifiedBuild;
use crate::bullets::{tick_own_bullets, tick_partner_bullets};
use crate::containers::tick as tick_containers;
use crate::cutscene_skip::tick as tick_cutscene_skip;
use crate::diagnostics::log_line;
use crate::dialogue::tick_dialogue;
use crate::door_sync::tick as tick_door_sync;
use crate::downed::tick_downed;
use crate::effects::tick as tick_effects;
use crate::enemy_bullets::tick as tick_enemy_bullets;
use crate::fade_sync::tick as tick_fade_sync;
use crate::fight_sync::tick as tick_fight_sync;
use crate::kaine::tick_swapped_spell_effects;
use crate::lift_rides::tick as tick_lift_rides;
use crate::music::tick as tick_music;
use crate::one_sided::tick as tick_one_sided;
use crate::pickups::tick as tick_pickups;
use crate::prologue_levels::tick as tick_prologue_levels;
use crate::puppet::{tick as tick_puppets, tick_ghost, tick_own_powers, tick_partner_powers};
use crate::script_rolls::tick as tick_script_rolls;
use crate::stay_together::tick as tick_stay_together;
use crate::story_jump::{run_story_jump, tick as tick_story_jump};
use crate::story_progress::tick as tick_story_progress;
use crate::story_relay::{tick as tick_story_relay, tick_relay_calls};
use crate::story_writes::tick as tick_story_writes;
use crate::teleport::{bring_guest_here, go_to_guest, tick as tick_teleport};

pub(super) fn run_queued_actions(build: &VerifiedBuild) {
    in_game_thread_step("script tick: queued actions", || run_actions(build));
    let steps: [(&'static str, fn(&VerifiedBuild)); 30] = [
        ("script tick: puppets", tick_puppets),
        ("script tick: ghost", tick_ghost),
        (
            "script tick: swapped spell effects",
            tick_swapped_spell_effects,
        ),
        ("script tick: own bullets", tick_own_bullets),
        ("script tick: own powers", tick_own_powers),
        ("script tick: partner bullets", tick_partner_bullets),
        ("script tick: enemy bullets", tick_enemy_bullets),
        ("script tick: partner powers", tick_partner_powers),
        ("script tick: effects", tick_effects),
        ("script tick: music", tick_music),
        ("script tick: downed", tick_downed),
        ("script tick: pickups", tick_pickups),
        ("script tick: stay together", tick_stay_together),
        ("script tick: dialogue", tick_dialogue),
        ("script tick: fade sync", tick_fade_sync),
        ("script tick: prologue levels", tick_prologue_levels),
        ("script tick: door sync", tick_door_sync),
        ("script tick: cutscene skip", tick_cutscene_skip),
        ("script tick: fight sync", tick_fight_sync),
        ("script tick: story writes", tick_story_writes),
        ("script tick: story relay", tick_story_relay),
        ("script tick: relay calls", tick_relay_calls),
        ("script tick: one sided", tick_one_sided),
        ("script tick: script rolls", tick_script_rolls),
        ("script tick: boss messages", tick_boss_messages),
        ("script tick: story jump", tick_story_jump),
        ("script tick: lift rides", tick_lift_rides),
        ("script tick: teleport", tick_teleport),
        ("script tick: story progress", tick_story_progress),
        ("script tick: containers", tick_containers),
    ];
    for (name, step) in steps {
        in_game_thread_step(name, || step(build));
    }
}

fn run_actions(build: &VerifiedBuild) {
    for action in take_all() {
        let outcome = match action {
            Action::ChangePhase { phase, point } => {
                change_phase(build, phase, point).map_err(|error| error.to_string())
            }
            Action::StoryJump(point) => run_story_jump(build, point),
            Action::BringGuestHere => bring_guest_here(build),
            Action::GoToGuest => go_to_guest(build),
            other => run_trainer_action(build, other).map_err(|error| error.to_string()),
        };
        match outcome {
            Ok(()) => log_line(&format!("action: {action:?}")),
            Err(error) => log_line(&format!("action: {action:?} failed ({error})")),
        }
    }
}
