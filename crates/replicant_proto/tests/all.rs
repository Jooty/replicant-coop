#[path = "attack_gauge_wire.rs"]
mod attack_gauge_wire;
#[path = "block_held_wire.rs"]
mod block_held_wire;
#[path = "block_moved_wire.rs"]
mod block_moved_wire;
#[path = "bomb_sync_wire.rs"]
mod bomb_sync_wire;
#[path = "bullet_flight_wire.rs"]
mod bullet_flight_wire;
#[path = "callback_fired_wire.rs"]
mod callback_fired_wire;
#[path = "cart_ride_wire.rs"]
mod cart_ride_wire;
#[path = "cart_run_wire.rs"]
mod cart_run_wire;
#[path = "container_wire.rs"]
mod container_wire;
#[path = "desync_wire.rs"]
mod desync_wire;
#[path = "dialogue_wire.rs"]
mod dialogue_wire;
#[path = "door_wire.rs"]
mod door_wire;
#[path = "effect_wire.rs"]
mod effect_wire;
#[path = "ending_records_wire.rs"]
mod ending_records_wire;
#[path = "enemy_bullet_flight_wire.rs"]
mod enemy_bullet_flight_wire;
#[path = "enemy_combat_wire.rs"]
mod enemy_combat_wire;
#[path = "enemy_creation_wire.rs"]
mod enemy_creation_wire;
#[path = "enemy_id_list_wire.rs"]
mod enemy_id_list_wire;
#[path = "enemy_wire.rs"]
mod enemy_wire;
#[path = "fight_camera_wire.rs"]
mod fight_camera_wire;
#[path = "fight_stream_wire.rs"]
mod fight_stream_wire;
#[path = "follow_phase.rs"]
mod follow_phase;
#[path = "game_saved_wire.rs"]
mod game_saved_wire;
#[path = "item_counts_wire.rs"]
mod item_counts_wire;
#[path = "lift_call_wire.rs"]
mod lift_call_wire;
#[path = "member_wire.rs"]
mod member_wire;
#[path = "message_handler_wire.rs"]
mod message_handler_wire;
#[path = "message_kind.rs"]
mod message_kind;
#[path = "music_wire.rs"]
mod music_wire;
#[path = "npc_wire.rs"]
mod npc_wire;
#[path = "partner_body_wire.rs"]
mod partner_body_wire;
#[path = "partner_book_wire.rs"]
mod partner_book_wire;
#[path = "paused_hits_wire.rs"]
mod paused_hits_wire;
#[path = "power_model_wire.rs"]
mod power_model_wire;
#[path = "prologue_level_wire.rs"]
mod prologue_level_wire;
#[path = "ridden_boar_wire.rs"]
mod ridden_boar_wire;
#[path = "scene_follow_wire.rs"]
mod scene_follow_wire;
#[path = "script_message_wire.rs"]
mod script_message_wire;
#[path = "script_roll_wire.rs"]
mod script_roll_wire;
#[path = "seat_wire.rs"]
mod seat_wire;
#[path = "shared_function_wire.rs"]
mod shared_function_wire;
#[path = "shot_cue_wire.rs"]
mod shot_cue_wire;
#[path = "sky_wire.rs"]
mod sky_wire;
#[path = "slot.rs"]
mod slot;
#[path = "story_call_wire.rs"]
mod story_call_wire;
#[path = "story_jump_wire.rs"]
mod story_jump_wire;
#[path = "story_wire.rs"]
mod story_wire;
#[path = "talk_choice_wire.rs"]
mod talk_choice_wire;
#[path = "track.rs"]
mod track;
#[path = "tutorial_prompt_wire.rs"]
mod tutorial_prompt_wire;
#[path = "voice_character_wire.rs"]
mod voice_character_wire;
#[path = "wire.rs"]
mod wire;
#[path = "world_effect_wire.rs"]
mod world_effect_wire;

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
