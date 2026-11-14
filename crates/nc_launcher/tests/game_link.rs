use nc_launcher::game_link::{
    clear_boot, clear_link, write_boot, write_coop_save_file, write_link, write_new_game,
    write_settings,
};
use nc_launcher_core::{PlayerSettings, REPLICANT, SessionOptions};

use crate::scratch::fresh_dir;

#[test]
fn writing_the_link_creates_the_file_with_port_and_name() {
    let dir = fresh_dir("link_write");
    write_link(&REPLICANT, &dir, 47200, "Ayla").unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(text.contains("link = 47200"));
    assert!(text.contains("name = \"Ayla\""));
}

#[test]
fn clearing_the_link_keeps_other_keys() {
    let dir = fresh_dir("link_keep");
    std::fs::write(dir.join(REPLICANT.link_file_name), "boot_slot = 1\n").unwrap();
    write_link(&REPLICANT, &dir, 5000, "Ayla").unwrap();
    clear_link(&REPLICANT, &dir).unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(text.contains("boot_slot = 1"));
    assert!(!text.contains("link"));
}

#[test]
fn clearing_a_link_only_file_removes_it() {
    let dir = fresh_dir("link_remove");
    write_link(&REPLICANT, &dir, 5000, "Ayla").unwrap();
    std::fs::write(dir.join(REPLICANT.link_file_name), "link = 5000\n").unwrap();
    clear_link(&REPLICANT, &dir).unwrap();
    assert!(!dir.join(REPLICANT.link_file_name).exists());
    clear_link(&REPLICANT, &dir).unwrap();
}

#[test]
fn boot_keys_are_written_and_other_keys_kept() {
    let dir = fresh_dir("boot_write");
    std::fs::write(dir.join(REPLICANT.link_file_name), "name = \"Ayla\"\n").unwrap();
    write_boot(&REPLICANT, &dir, Some(3), true).unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(text.contains("name = \"Ayla\""));
    assert!(text.contains("boot_slot = 3"));
    assert!(text.contains("block_saves = true"));
}

#[test]
fn a_host_boot_removes_a_stale_save_block() {
    let dir = fresh_dir("boot_host");
    write_boot(&REPLICANT, &dir, Some(1), true).unwrap();
    write_boot(&REPLICANT, &dir, None, false).unwrap();
    assert!(
        !dir.join(REPLICANT.link_file_name).exists() || {
            let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
            !text.contains("block_saves") && !text.contains("boot_slot")
        }
    );
}

#[test]
fn clearing_boot_keeps_the_link() {
    let dir = fresh_dir("boot_clear");
    write_link(&REPLICANT, &dir, 5000, "Ayla").unwrap();
    write_boot(&REPLICANT, &dir, Some(0), true).unwrap();
    clear_boot(&REPLICANT, &dir).unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(text.contains("link = 5000"));
    assert!(!text.contains("boot_slot"));
}

#[test]
fn log_path_is_written_and_cleared_with_the_boot_settings() {
    let dir = fresh_dir("link_log");
    write_link(&REPLICANT, &dir, 5000, "Ayla").unwrap();
    nc_launcher::game_link::write_log(&REPLICANT, &dir, "Z:\\logs\\game-host.log").unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(text.contains("log = "));
    clear_boot(&REPLICANT, &dir).unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(!text.contains("log"));
    assert!(text.contains("link = 5000"));
}

#[test]
fn only_a_dev_build_writes_the_focus_menu_switch_and_clearing_boot_removes_it() {
    let dir = fresh_dir("dev_switches");
    write_link(&REPLICANT, &dir, 5000, "Ayla").unwrap();
    nc_launcher::game_link::write_dev_switches(&REPLICANT, &dir, false).unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(!text.contains("dev_no_focus_menu"));
    nc_launcher::game_link::write_dev_switches(&REPLICANT, &dir, true).unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(text.contains("dev_no_focus_menu = true"));
    clear_boot(&REPLICANT, &dir).unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(!text.contains("dev_no_focus_menu"));
}

#[test]
fn settings_are_written_once_and_defaults_leave_no_key() {
    let dir = fresh_dir("settings_write");
    let settings = PlayerSettings {
        keep_running: true,
        event_log: false,
    };
    let options = SessionOptions {
        show_names: true,
        show_party_frame: false,
        show_distance: true,
        show_ping: false,
        ..SessionOptions::default()
    };
    write_settings(&REPLICANT, &dir, &settings, &options).unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(text.contains("show_distance = true"));
    assert!(text.contains("keep_running = true"));
    assert!(text.contains("event_log = false"));
    assert!(text.contains("show_party_frame = false"));
    assert!(!text.contains("show_ping"));
    assert!(!text.contains("show_names"));
}

#[test]
fn settings_survive_clearing_the_boot_keys_except_keep_running() {
    let dir = fresh_dir("settings_clear");
    let settings = PlayerSettings {
        keep_running: true,
        event_log: false,
    };
    let options = SessionOptions {
        show_distance: true,
        show_ping: true,
        ..SessionOptions::default()
    };
    write_settings(&REPLICANT, &dir, &settings, &options).unwrap();
    clear_boot(&REPLICANT, &dir).unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(text.contains("show_distance = true"));
    assert!(text.contains("show_ping = true"));
    assert!(text.contains("event_log = false"));
    assert!(!text.contains("keep_running"));
}

#[test]
fn turning_a_setting_back_to_its_default_removes_its_key_and_keeps_the_rest() {
    let dir = fresh_dir("settings_default");
    write_link(&REPLICANT, &dir, 5000, "Ayla").unwrap();
    let mut options = SessionOptions {
        show_distance: true,
        ..SessionOptions::default()
    };
    write_settings(&REPLICANT, &dir, &PlayerSettings::default(), &options).unwrap();
    options.show_distance = false;
    write_settings(&REPLICANT, &dir, &PlayerSettings::default(), &options).unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(!text.contains("show_distance"));
    assert!(text.contains("link = 5000"));
}

#[test]
fn enemy_difficulty_is_written_only_when_it_is_not_the_default() {
    let dir = fresh_dir("settings_difficulty");
    let options = SessionOptions {
        enemy_health_percent: 120,
        ..SessionOptions::default()
    };
    write_settings(&REPLICANT, &dir, &PlayerSettings::default(), &options).unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(text.contains("enemy_health_percent = 120"));
    assert!(!text.contains("enemy_damage_percent"));
}

#[test]
fn a_new_game_round_trips_and_replaces_a_boot_slot() {
    let dir = fresh_dir("new_game_write");
    write_boot(&REPLICANT, &dir, Some(1), true).unwrap();
    write_new_game(&REPLICANT, &dir, 2, "Ayla", true).unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(text.contains("new_game_slot = 2"));
    assert!(text.contains("new_game_name = \"Ayla\""));
    assert!(text.contains("block_saves = true"));
    assert!(!text.contains("boot_slot"));
    write_boot(&REPLICANT, &dir, Some(0), true).unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(text.contains("boot_slot = 0"));
    assert!(!text.contains("new_game"));
}

#[test]
fn clearing_boot_removes_the_new_game_keys() {
    let dir = fresh_dir("new_game_clear");
    write_link(&REPLICANT, &dir, 5000, "Ayla").unwrap();
    write_new_game(&REPLICANT, &dir, 0, "Ayla", true).unwrap();
    clear_boot(&REPLICANT, &dir).unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(text.contains("link = 5000"));
    assert!(!text.contains("new_game"));
    assert!(!text.contains("block_saves"));
}

fn written_text(dir: &std::path::Path) -> String {
    std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap()
}

#[test]
fn a_boot_clears_a_body_key_an_older_launcher_left_in_the_file() {
    let dir = fresh_dir("body_key_cleared");
    std::fs::write(
        dir.join(REPLICANT.link_file_name),
        "link = 5000\nbody = \"prologue\"\n",
    )
    .unwrap();
    write_boot(&REPLICANT, &dir, Some(1), false).unwrap();
    assert!(!written_text(&dir).contains("body"));
    std::fs::write(
        dir.join(REPLICANT.link_file_name),
        "link = 5000\nbody = \"kaine\"\n",
    )
    .unwrap();
    write_new_game(&REPLICANT, &dir, 2, "Ayla", false).unwrap();
    assert!(!written_text(&dir).contains("body"));
}

#[test]
fn a_continue_launch_clears_stale_new_game_keys() {
    let dir = fresh_dir("continue_clears_new_game");
    write_new_game(&REPLICANT, &dir, 2, "Ayla", false).unwrap();
    write_boot(&REPLICANT, &dir, Some(1), false).unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(text.contains("boot_slot = 1"));
    assert!(!text.contains("new_game"));
}

#[test]
fn a_plain_launch_clears_stale_new_game_keys() {
    let dir = fresh_dir("plain_clears_new_game");
    write_new_game(&REPLICANT, &dir, 2, "Ayla", false).unwrap();
    write_boot(&REPLICANT, &dir, None, false).unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(!text.contains("new_game"));
    assert!(!text.contains("boot_slot"));
}

#[test]
fn the_coop_save_file_is_written_only_when_named_and_cleared_with_the_boot() {
    let dir = fresh_dir("coop_save_file");
    write_link(&REPLICANT, &dir, 5000, "Ayla").unwrap();
    write_coop_save_file(&REPLICANT, &dir, None).unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(!text.contains("coop_save_file"));
    write_coop_save_file(&REPLICANT, &dir, Some("GAMEDATA.coop.seat")).unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(text.contains("coop_save_file = \"GAMEDATA.coop.seat\""));
    write_coop_save_file(&REPLICANT, &dir, None).unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(!text.contains("coop_save_file"));
    write_coop_save_file(&REPLICANT, &dir, Some("GAMEDATA.coop.seat")).unwrap();
    clear_boot(&REPLICANT, &dir).unwrap();
    let text = std::fs::read_to_string(dir.join(REPLICANT.link_file_name)).unwrap();
    assert!(text.contains("link = 5000"));
    assert!(!text.contains("coop_save_file"));
}
