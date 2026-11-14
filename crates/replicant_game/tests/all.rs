mod fake_memory;

#[path = "actor_model.rs"]
mod actor_model;
#[path = "arrival.rs"]
mod arrival;
#[path = "arrive_in_state.rs"]
mod arrive_in_state;
#[path = "attack_gauge.rs"]
mod attack_gauge;
#[path = "attack_gauge_opening_hit.rs"]
mod attack_gauge_opening_hit;
#[path = "attack_gauge_shared.rs"]
mod attack_gauge_shared;
#[path = "blocks.rs"]
mod blocks;
#[path = "body_following_camera.rs"]
mod body_following_camera;
#[path = "bomb_tracking.rs"]
mod bomb_tracking;
#[path = "bomb_walls.rs"]
mod bomb_walls;
#[path = "boss_bar_probe.rs"]
mod boss_bar_probe;
#[path = "boss_hit_log.rs"]
mod boss_hit_log;
#[path = "boss_messages.rs"]
mod boss_messages;
#[path = "bullet_flights.rs"]
mod bullet_flights;
#[path = "bullet_hits.rs"]
mod bullet_hits;
#[path = "ca_gauge.rs"]
mod ca_gauge;
#[path = "callback_gate.rs"]
mod callback_gate;
#[path = "camera_aim_follow.rs"]
mod camera_aim_follow;
#[path = "camera_control.rs"]
mod camera_control;
#[path = "camera_follow.rs"]
mod camera_follow;
#[path = "cart_steering.rs"]
mod cart_steering;
#[path = "carts.rs"]
mod carts;
#[path = "choice_hover.rs"]
mod choice_hover;
#[path = "companion.rs"]
mod companion;
#[path = "containers.rs"]
mod containers;
#[path = "copy_hold.rs"]
mod copy_hold;
#[path = "cube_probe.rs"]
mod cube_probe;
#[path = "cutscene_skip.rs"]
mod cutscene_skip;
#[path = "desync.rs"]
mod desync;
#[path = "dialogue.rs"]
mod dialogue;
#[path = "door_sync.rs"]
mod door_sync;
#[path = "downed.rs"]
mod downed;
#[path = "dummy_hits.rs"]
mod dummy_hits;
#[path = "effect_look.rs"]
mod effect_look;
#[path = "effect_push.rs"]
mod effect_push;
#[path = "effect_source.rs"]
mod effect_source;
#[path = "effects.rs"]
mod effects;
#[path = "ending_records.rs"]
mod ending_records;
#[path = "enemies.rs"]
mod enemies;
#[path = "enemy_bullets.rs"]
mod enemy_bullets;
#[path = "enemy_combat.rs"]
mod enemy_combat;
#[path = "enemy_creation.rs"]
mod enemy_creation;
#[path = "enemy_difficulty.rs"]
mod enemy_difficulty;
#[path = "enemy_fight_waves.rs"]
mod enemy_fight_waves;
#[path = "enemy_log.rs"]
mod enemy_log;
#[path = "enemy_roster_cap.rs"]
mod enemy_roster_cap;
#[path = "enemy_takeover.rs"]
mod enemy_takeover;
#[path = "entity.rs"]
mod entity;
#[path = "event_log.rs"]
mod event_log;
#[path = "eye_set_tally.rs"]
mod eye_set_tally;
#[path = "far_guest_follow.rs"]
mod far_guest_follow;
#[path = "fight_doors.rs"]
mod fight_doors;
#[path = "fight_entry_end.rs"]
mod fight_entry_end;
#[path = "fight_player_traps.rs"]
mod fight_player_traps;
#[path = "fight_requests.rs"]
mod fight_requests;
#[path = "fight_start_entry.rs"]
mod fight_start_entry;
#[path = "fight_stream.rs"]
mod fight_stream;
#[path = "fight_sync.rs"]
mod fight_sync;
#[path = "fight_tape.rs"]
mod fight_tape;
#[path = "fight_window.rs"]
mod fight_window;
#[path = "fill_in.rs"]
mod fill_in;
#[path = "fix_buttons.rs"]
mod fix_buttons;
#[path = "focus.rs"]
mod focus;
#[path = "frame_ring.rs"]
mod frame_ring;
#[path = "game_closing.rs"]
mod game_closing;
#[path = "hit_kind.rs"]
mod hit_kind;
#[path = "hit_reaction.rs"]
mod hit_reaction;
#[path = "hit_stagger.rs"]
mod hit_stagger;
#[path = "host_led_runs.rs"]
mod host_led_runs;
#[path = "host_led_script_talk.rs"]
mod host_led_script_talk;
#[path = "host_removal.rs"]
mod host_removal;
#[path = "inventory.rs"]
mod inventory;
#[path = "invincibility.rs"]
mod invincibility;
#[path = "kaine_companion_off.rs"]
mod kaine_companion_off;
#[path = "kaine_companion_off_fallback.rs"]
mod kaine_companion_off_fallback;
#[path = "kaine_hit_power.rs"]
mod kaine_hit_power;
#[path = "kaine_level_in_use.rs"]
mod kaine_level_in_use;
#[path = "kaine_partner.rs"]
mod kaine_partner;
#[path = "kaine_rows_from_scripts.rs"]
mod kaine_rows_from_scripts;
#[path = "kaine_save.rs"]
mod kaine_save;
#[path = "kaine_scene_stand_in.rs"]
mod kaine_scene_stand_in;
#[path = "kaine_spell_brightness.rs"]
mod kaine_spell_brightness;
#[path = "kaine_spell_effect_tally.rs"]
mod kaine_spell_effect_tally;
#[path = "kaine_spell_look.rs"]
mod kaine_spell_look;
#[path = "kaine_spell_size.rs"]
mod kaine_spell_size;
#[path = "kaine_stand_in.rs"]
mod kaine_stand_in;
#[path = "kaine_stats.rs"]
mod kaine_stats;
#[path = "kaine_swapped_level.rs"]
mod kaine_swapped_level;
#[path = "kill_guard.rs"]
mod kill_guard;
#[path = "lift_rides.rs"]
mod lift_rides;
#[path = "local_pose.rs"]
mod local_pose;
#[path = "message_handlers.rs"]
mod message_handlers;
#[path = "minimap.rs"]
mod minimap;
#[path = "move_log.rs"]
mod move_log;
#[path = "music.rs"]
mod music;
#[path = "new_game.rs"]
mod new_game;
#[path = "noclip.rs"]
mod noclip;
#[path = "npcs.rs"]
mod npcs;
#[path = "one_sided.rs"]
mod one_sided;
#[path = "own_hit_log.rs"]
mod own_hit_log;
#[path = "partner_holds.rs"]
mod partner_holds;
#[path = "partner_marker.rs"]
mod partner_marker;
#[path = "party_frame.rs"]
mod party_frame;
#[path = "patches.rs"]
mod patches;
#[path = "pe_identity.rs"]
mod pe_identity;
#[path = "phase_state.rs"]
mod phase_state;
#[path = "phases.rs"]
mod phases;
#[path = "pickups.rs"]
mod pickups;
#[path = "player_name.rs"]
mod player_name;
#[path = "player_stats.rs"]
mod player_stats;
#[path = "player_waits.rs"]
mod player_waits;
#[path = "popup_saved_mask.rs"]
mod popup_saved_mask;
#[path = "position.rs"]
mod position;
#[path = "power_models.rs"]
mod power_models;
#[path = "probe_log.rs"]
mod probe_log;
#[path = "prologue_levels.rs"]
mod prologue_levels;
#[path = "prompt_follow.rs"]
mod prompt_follow;
#[path = "prompt_follow_partner_led.rs"]
mod prompt_follow_partner_led;
#[path = "puppet.rs"]
mod puppet;
#[path = "puppet_body.rs"]
mod puppet_body;
#[path = "puppet_book.rs"]
mod puppet_book;
#[path = "puppet_draw_report.rs"]
mod puppet_draw_report;
#[path = "puppet_weapon.rs"]
mod puppet_weapon;
#[path = "quiet_end.rs"]
mod quiet_end;
#[path = "radio_pause.rs"]
mod radio_pause;
#[path = "radio_waits.rs"]
mod radio_waits;
#[path = "readiness.rs"]
mod readiness;
#[path = "relay_call_gate.rs"]
mod relay_call_gate;
#[path = "relayed_scenes.rs"]
mod relayed_scenes;
#[path = "revive_display.rs"]
mod revive_display;
#[path = "rewind.rs"]
mod rewind;
#[path = "ridden_boar.rs"]
mod ridden_boar;
#[path = "save_files.rs"]
mod save_files;
#[path = "scene_follow.rs"]
mod scene_follow;
#[path = "scene_gather.rs"]
mod scene_gather;
#[path = "scene_pull.rs"]
mod scene_pull;
#[path = "script_functions.rs"]
mod script_functions;
#[path = "script_jump.rs"]
mod script_jump;
#[path = "script_pause.rs"]
mod script_pause;
#[path = "script_rolls.rs"]
mod script_rolls;
#[path = "script_runner.rs"]
mod script_runner;
#[path = "scripted_camera.rs"]
mod scripted_camera;
#[path = "seat.rs"]
mod seat;
#[path = "shared_events.rs"]
mod shared_events;
#[path = "shared_items.rs"]
mod shared_items;
#[path = "shot_copies.rs"]
mod shot_copies;
#[path = "shot_cues.rs"]
mod shot_cues;
#[path = "side_view.rs"]
mod side_view;
#[path = "sky.rs"]
mod sky;
#[path = "stay_together.rs"]
mod stay_together;
#[path = "story_calls.rs"]
mod story_calls;
#[path = "story_jump.rs"]
mod story_jump;
#[path = "story_player_local_flags.rs"]
mod story_player_local_flags;
#[path = "story_progress.rs"]
mod story_progress;
#[path = "story_relay.rs"]
mod story_relay;
#[path = "story_writes.rs"]
mod story_writes;
#[path = "stuck_menu_mask.rs"]
mod stuck_menu_mask;
#[path = "talk_guard.rs"]
mod talk_guard;
#[path = "talk_refusal.rs"]
mod talk_refusal;
#[path = "teleport_check.rs"]
mod teleport_check;
#[path = "time_scale.rs"]
mod time_scale;
#[path = "tutorial_prompt.rs"]
mod tutorial_prompt;
#[path = "ui_scenes.rs"]
mod ui_scenes;
#[path = "view_list.rs"]
mod view_list;
#[path = "voice_character.rs"]
mod voice_character;
#[path = "world_effects.rs"]
mod world_effects;
#[path = "zone_rules.rs"]
mod zone_rules;

use std::collections::BTreeSet;
use std::path::Path;

#[test]
fn every_test_file_is_part_of_the_suite() {
    let tests_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let suite = include_str!("all.rs");
    let missing: BTreeSet<String> = std::fs::read_dir(&tests_dir)
        .expect("tests directory is readable")
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter_map(|name| name.strip_suffix(".rs").map(str::to_owned))
        .filter(|name| name != "all" && !suite.contains(&format!("mod {name};")))
        .collect();
    assert!(
        missing.is_empty(),
        "test files missing from tests/all.rs: {missing:?}"
    );
}
