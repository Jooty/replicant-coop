use std::fs;
use std::path::PathBuf;

use replicant_hook::net::{
    AUTO_CONFIG_FILE_NAME, AutoConfig, AutoConfigError, DEFAULT_LOG_FILE_NAME, resolve_log_path,
};

fn scratch_folder(test_name: &str) -> PathBuf {
    let folder = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("test-scratch")
        .join(test_name);
    fs::create_dir_all(&folder).expect("scratch folder is creatable");
    folder
}

#[test]
fn a_link_port_and_name_are_read() {
    let config = AutoConfig::parse("link = 47200\nname = \"Ayla\"\n").expect("valid");
    assert_eq!(config.link, Some(47200));
    assert_eq!(config.name.as_deref(), Some("Ayla"));
}

#[test]
fn a_body_key_left_in_an_old_link_file_is_read_and_changes_nothing() {
    for old in ["nier", "prologue", "kaine", "kaineE", "somebody"] {
        let config =
            AutoConfig::parse(&format!("link = 47200\nbody = \"{old}\"\n")).expect("valid");
        assert_eq!(config.link, Some(47200));
    }
}

#[test]
fn the_companion_rows_from_the_scripts_are_on_unless_switched_off() {
    let unset = AutoConfig::parse("link = 47200\n").expect("valid");
    assert!(unset.companion_rows_from_scripts_enabled());
    let off =
        AutoConfig::parse("link = 47200\ncompanion_rows_from_scripts = false\n").expect("valid");
    assert!(!off.companion_rows_from_scripts_enabled());
    let on = AutoConfig::parse("companion_rows_from_scripts = true\n").expect("valid");
    assert!(on.companion_rows_from_scripts_enabled());
}

#[test]
fn the_name_is_optional() {
    let config = AutoConfig::parse("link = 47200\n").expect("valid");
    assert_eq!(config.name, None);
}

#[test]
fn port_zero_is_invalid() {
    assert!(matches!(
        AutoConfig::parse("link = 0\n"),
        Err(AutoConfigError::Invalid(_))
    ));
}

#[test]
fn the_old_in_game_session_keys_are_invalid() {
    assert!(matches!(
        AutoConfig::parse("link = 1\nmode = \"host\"\n"),
        Err(AutoConfigError::Invalid(_))
    ));
    assert!(matches!(
        AutoConfig::parse("link = 1\nticket = \"rcn1abc\"\n"),
        Err(AutoConfigError::Invalid(_))
    ));
}

#[test]
fn missing_file_means_no_launcher_link() {
    let folder = scratch_folder("auto_config_missing");
    let _ = fs::remove_file(folder.join(AUTO_CONFIG_FILE_NAME));
    assert_eq!(AutoConfig::load_from_folder(&folder), Ok(None));
}

#[test]
fn file_in_the_game_folder_is_loaded() {
    let folder = scratch_folder("auto_config_present");
    fs::write(folder.join(AUTO_CONFIG_FILE_NAME), "link = 47200\n").expect("writable");
    let config = AutoConfig::load_from_folder(&folder)
        .expect("valid file")
        .expect("file exists");
    assert_eq!(config.link, Some(47200));
}

#[test]
fn boot_slot_and_block_saves_are_read() {
    let config = AutoConfig::parse("boot_slot = 2\nblock_saves = true\n").expect("valid");
    assert_eq!(config.link, None);
    assert_eq!(config.boot_slot, Some(2));
    assert!(config.block_saves);
}

#[test]
fn boot_slot_and_block_saves_default_to_off() {
    let config = AutoConfig::parse("link = 47200\n").expect("valid");
    assert_eq!(config.boot_slot, None);
    assert!(!config.block_saves);
}

#[test]
fn a_boot_slot_past_the_last_save_slot_is_invalid() {
    assert!(matches!(
        AutoConfig::parse("boot_slot = 32\n"),
        Err(AutoConfigError::Invalid(_))
    ));
}

fn environment(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
    let owned: Vec<(String, String)> = pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    move |key| {
        owned
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.clone())
    }
}

#[test]
fn environment_overrides_the_file() {
    let file = AutoConfig::parse("link = 47200\nname = \"Ayla\"\nboot_slot = 1\n").expect("valid");
    let config = AutoConfig::layered(
        Some(file),
        environment(&[
            ("NIER_COOP_LINK", "47300"),
            ("NIER_COOP_BOOT_SLOT", "3"),
            ("NIER_COOP_BLOCK_SAVES", "1"),
            ("NIER_COOP_NAME", "Guest"),
        ]),
    )
    .expect("valid")
    .expect("present");
    assert_eq!(config.link, Some(47300));
    assert_eq!(config.boot_slot, Some(3));
    assert!(config.block_saves);
    assert_eq!(config.name.as_deref(), Some("Guest"));
}

#[test]
fn file_values_stay_where_the_environment_is_silent() {
    let file = AutoConfig::parse("link = 47200\nname = \"Ayla\"\n").expect("valid");
    let config = AutoConfig::layered(Some(file), environment(&[("NIER_COOP_BOOT_SLOT", "2")]))
        .expect("valid")
        .expect("present");
    assert_eq!(config.link, Some(47200));
    assert_eq!(config.name.as_deref(), Some("Ayla"));
    assert_eq!(config.boot_slot, Some(2));
}

#[test]
fn environment_can_turn_a_file_block_off() {
    let file = AutoConfig::parse("link = 47200\nblock_saves = true\n").expect("valid");
    let config = AutoConfig::layered(
        Some(file),
        environment(&[("NIER_COOP_BLOCK_SAVES", "false")]),
    )
    .expect("valid")
    .expect("present");
    assert!(!config.block_saves);
}

#[test]
fn environment_alone_makes_a_config() {
    let config = AutoConfig::layered(None, environment(&[("NIER_COOP_LINK", "47300")]))
        .expect("valid")
        .expect("present");
    assert_eq!(config.link, Some(47300));
}

#[test]
fn no_file_and_no_environment_means_no_config() {
    assert_eq!(AutoConfig::layered(None, environment(&[])), Ok(None));
}

#[test]
fn bad_environment_values_are_invalid() {
    for pairs in [
        &[("NIER_COOP_LINK", "abc")][..],
        &[("NIER_COOP_LINK", "0")],
        &[("NIER_COOP_BOOT_SLOT", "99")],
        &[("NIER_COOP_BLOCK_SAVES", "maybe")],
    ] {
        assert!(matches!(
            AutoConfig::layered(None, environment(pairs)),
            Err(AutoConfigError::Invalid(_))
        ));
    }
}

#[test]
fn load_layered_reads_the_file_then_the_environment() {
    let folder = scratch_folder("auto_config_layered");
    fs::write(folder.join(AUTO_CONFIG_FILE_NAME), "link = 47200\n").expect("writable");
    let config = AutoConfig::load_layered(&folder, environment(&[("NIER_COOP_LINK", "47301")]))
        .expect("valid")
        .expect("present");
    assert_eq!(config.link, Some(47301));
}

#[test]
fn log_path_comes_from_the_file_or_the_environment() {
    let from_file = AutoConfig::parse("log = \"Z:\\\\logs\\\\game-host.log\"\n").expect("valid");
    assert_eq!(from_file.log.as_deref(), Some("Z:\\logs\\game-host.log"));
    let from_environment = AutoConfig::layered(
        Some(from_file),
        environment(&[("NIER_COOP_LOG", "Z:\\logs\\game-guest.log")]),
    )
    .expect("valid")
    .expect("present");
    assert_eq!(
        from_environment.log.as_deref(),
        Some("Z:\\logs\\game-guest.log")
    );
}

#[test]
fn log_path_prefers_environment_then_file_then_game_folder() {
    let env = Some("Z:\\a.log".to_owned());
    assert_eq!(
        resolve_log_path(env, || Some("b.log".to_owned())),
        "Z:\\a.log"
    );
    assert_eq!(resolve_log_path(None, || Some("b.log".to_owned())), "b.log");
    assert_eq!(
        resolve_log_path(Some("  ".to_owned()), || None),
        DEFAULT_LOG_FILE_NAME
    );
}

#[test]
fn keep_running_is_read_and_defaults_to_off() {
    let on = AutoConfig::parse("keep_running = true\n").expect("valid");
    assert!(on.keep_running);
    let off = AutoConfig::parse("link = 47200\n").expect("valid");
    assert!(!off.keep_running);
}

#[test]
fn the_dev_focus_menu_switch_is_read_and_defaults_to_off() {
    let on = AutoConfig::parse("dev_no_focus_menu = true\n").expect("valid");
    assert!(on.dev_no_focus_menu);
    let off = AutoConfig::parse("link = 47200\n").expect("valid");
    assert!(!off.dev_no_focus_menu);
}

#[test]
fn tag_options_default_to_off_and_are_read() {
    let off = AutoConfig::parse("link = 5000\n").expect("valid");
    assert!(!off.show_distance && !off.show_ping);
    let on = AutoConfig::parse("show_distance = true\nshow_ping = true\n").expect("valid");
    assert!(on.show_distance && on.show_ping);
}

#[test]
fn the_event_log_and_host_options_are_on_unless_the_file_turns_them_off() {
    let plain = AutoConfig::parse("link = 5000\n").expect("valid");
    assert!(plain.event_log_enabled());
    assert_eq!(plain.show_names, None);
    let off =
        AutoConfig::parse("event_log = false\nshow_names = false\nshow_party_frame = false\n")
            .expect("valid");
    assert!(!off.event_log_enabled());
    assert_eq!(off.show_names, Some(false));
    assert_eq!(off.show_party_frame, Some(false));
}

#[test]
fn new_game_keys_are_read() {
    let config = AutoConfig::parse("new_game_slot = 2\nnew_game_name = \"Ayla\"\n").expect("valid");
    assert_eq!(config.new_game_slot, Some(2));
    assert_eq!(config.new_game_name.as_deref(), Some("Ayla"));
    assert_eq!(config.boot_slot, None);
}

#[test]
fn a_new_game_slot_past_the_sixth_is_invalid() {
    assert!(matches!(
        AutoConfig::parse("new_game_slot = 6\n"),
        Err(AutoConfigError::Invalid(_))
    ));
}

#[test]
fn a_new_game_and_a_boot_slot_exclude_each_other() {
    assert!(matches!(
        AutoConfig::parse("new_game_slot = 1\nboot_slot = 1\n"),
        Err(AutoConfigError::Invalid(_))
    ));
}

#[test]
fn the_environment_sets_the_new_game_keys() {
    let config = AutoConfig::layered(
        None,
        environment(&[
            ("NIER_COOP_NEW_GAME_SLOT", "3"),
            ("NIER_COOP_NEW_GAME_NAME", "Ayla"),
        ]),
    )
    .expect("valid")
    .expect("present");
    assert_eq!(config.new_game_slot, Some(3));
    assert_eq!(config.new_game_name.as_deref(), Some("Ayla"));
}

#[test]
fn the_environment_cannot_mix_a_new_game_with_a_file_boot_slot() {
    let file = AutoConfig::parse("boot_slot = 1\n").expect("valid");
    assert!(matches!(
        AutoConfig::layered(Some(file), environment(&[("NIER_COOP_NEW_GAME_SLOT", "0")])),
        Err(AutoConfigError::Invalid(_))
    ));
}

#[test]
fn the_old_body_environment_variable_is_not_read() {
    let config =
        AutoConfig::layered(None, environment(&[("NIER_COOP_BODY", "prologue")])).expect("valid");
    assert_eq!(config, None);
}

#[test]
fn the_coop_save_file_is_read_and_defaults_to_none() {
    let named = AutoConfig::parse("coop_save_file = \"GAMEDATA.coop.seat\"\n").expect("valid");
    let unset = AutoConfig::parse("").expect("valid");
    assert_eq!(named.coop_save_file.as_deref(), Some("GAMEDATA.coop.seat"));
    assert_eq!(unset.coop_save_file, None);
}

#[test]
fn an_unusable_coop_save_file_still_reads_so_the_mod_can_refuse_it() {
    let config =
        AutoConfig::parse("link = 5000\ncoop_save_file = \"../GAMEDATA\"\n").expect("valid");
    assert_eq!(config.link, Some(5000));
    assert_eq!(config.coop_save_file.as_deref(), Some("../GAMEDATA"));
}
