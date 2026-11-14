use std::path::{Path, PathBuf};

use crate::scratch::fresh_dir;
use nc_launcher::second_copy::{
    ProtonSetup, copy_command, copy_environment, proton_script_from_config_info,
    required_tool_app_id, steam_client_root,
};
use nc_launcher::steam::GameInstall;
use nc_launcher_core::REPLICANT;

fn install() -> GameInstall {
    GameInstall {
        profile: &REPLICANT,
        game_dir: PathBuf::from("/lib/steamapps/common/NieR"),
        steamapps_dir: PathBuf::from("/lib/steamapps"),
        proton_prefix: None,
    }
}

fn value<'a>(envs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    envs.iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
}

#[test]
fn the_second_copy_is_a_guest_with_its_own_link() {
    let envs = copy_environment(47300, "Guest", Some(2), None);
    assert_eq!(value(&envs, "NIER_COOP_LINK"), Some("47300"));
    assert_eq!(value(&envs, "NIER_COOP_NAME"), Some("Guest"));
    assert_eq!(value(&envs, "NIER_COOP_BOOT_SLOT"), Some("2"));
    assert_eq!(value(&envs, "NIER_COOP_BLOCK_SAVES"), Some("1"));
}

#[test]
fn no_slot_means_no_boot_slot_variable() {
    let envs = copy_environment(47300, "Guest", None, None);
    assert_eq!(value(&envs, "NIER_COOP_BOOT_SLOT"), None);
}

#[test]
fn without_proton_the_exe_starts_directly() {
    let command = copy_command(&install(), copy_environment(1, "G", None, None), None);
    assert_eq!(command.program, install().exe_path());
    assert!(command.args.is_empty());
    assert_eq!(command.current_dir, install().game_dir);
    assert_eq!(value(&command.envs, "SteamAppId"), None);
}

fn setup(runtime_entry: Option<&str>) -> ProtonSetup {
    ProtonSetup {
        script: PathBuf::from("/lib/steamapps/common/Proton 9.0/proton"),
        steam_root: PathBuf::from("/home/u/.local/share/Steam"),
        runtime_entry: runtime_entry.map(PathBuf::from),
    }
}

#[test]
fn with_proton_the_exe_runs_through_the_proton_script() {
    let proton = setup(None);
    let command = copy_command(
        &install(),
        copy_environment(1, "G", None, None),
        Some(&proton),
    );
    assert_eq!(command.program, proton.script);
    assert_eq!(command.args[0], "run");
    assert_eq!(Path::new(&command.args[1]), install().exe_path());
    assert!(
        command.args[1]
            .to_string_lossy()
            .ends_with(REPLICANT.exe_names[0])
    );
    assert_eq!(
        value(&command.envs, "STEAM_COMPAT_DATA_PATH"),
        Some("/lib/steamapps/compatdata/1113560")
    );
    assert_eq!(
        value(&command.envs, "STEAM_COMPAT_CLIENT_INSTALL_PATH"),
        Some("/home/u/.local/share/Steam")
    );
    assert_eq!(value(&command.envs, "SteamAppId"), Some("1113560"));
    assert_eq!(value(&command.envs, "SteamGameId"), Some("1113560"));
    assert_eq!(value(&command.envs, "NIER_COOP_LINK"), Some("1"));
}

#[test]
fn a_proton_that_needs_a_runtime_starts_inside_its_entry_point() {
    let proton = setup(Some("/rt/_v2-entry-point"));
    let command = copy_command(
        &install(),
        copy_environment(1, "G", None, None),
        Some(&proton),
    );
    assert_eq!(command.program, PathBuf::from("/rt/_v2-entry-point"));
    assert_eq!(command.args[0], "--verb=run");
    assert_eq!(command.args[1], "--");
    assert_eq!(Path::new(&command.args[2]), proton.script);
    assert_eq!(command.args[3], "run");
    assert_eq!(
        value(&command.envs, "STEAM_COMPAT_LIBRARY_PATHS"),
        Some("/lib/steamapps")
    );
    assert_eq!(
        value(&command.envs, "STEAM_COMPAT_TOOL_PATHS"),
        Some("/lib/steamapps/common/Proton 9.0:/rt")
    );
}

#[test]
fn the_launch_line_quotes_paths_with_spaces() {
    let proton = setup(None);
    let command = copy_command(
        &install(),
        copy_environment(1, "G H", None, None),
        Some(&proton),
    );
    let line = command.shell_line();
    assert!(line.contains("NIER_COOP_NAME='G H'"));
    assert!(line.contains("'/lib/steamapps/common/Proton 9.0/proton' 'run'"));
}

#[test]
fn the_required_runtime_is_read_from_the_tool_manifest() {
    let manifest =
        "\"manifest\"\n{\n  \"version\" \"2\"\n  \"require_tool_appid\" \"4183110\"\n}\n";
    assert_eq!(required_tool_app_id(manifest), Some("4183110".to_owned()));
    assert_eq!(required_tool_app_id("\"manifest\"\n{\n}\n"), None);
}

#[test]
fn the_steam_folder_is_never_the_game_library() {
    let root = fresh_dir("second_copy_steam_root");
    let library = fresh_dir("second_copy_steam_library");
    std::fs::write(root.join("steam.sh"), "").expect("write steam.sh");
    let info = format!("9.0\n{}\n/x/files/share/\n", library.display());
    assert_eq!(
        steam_client_root(&info, std::slice::from_ref(&library)),
        None
    );
    let info = format!("9.0\n/x/files/share/\n{}\n", root.display());
    assert_eq!(steam_client_root(&info, &[library]), Some(root));
}

#[test]
fn the_proton_script_sits_beside_the_files_folder() {
    let info = "9.0-4\n1700000000\n/home/u/steam/common/Proton 9.0/files/share/default_pfx/\n";
    assert_eq!(
        proton_script_from_config_info(info),
        Some(PathBuf::from("/home/u/steam/common/Proton 9.0/proton"))
    );
    assert_eq!(proton_script_from_config_info("nothing"), None);
}

#[test]
fn a_new_game_passes_its_slot_and_name() {
    let envs = copy_environment(47300, "Guest", None, Some((4, "Ayla")));
    assert_eq!(value(&envs, "NIER_COOP_NEW_GAME_SLOT"), Some("4"));
    assert_eq!(value(&envs, "NIER_COOP_NEW_GAME_NAME"), Some("Ayla"));
}

#[test]
fn no_new_game_means_no_new_game_variables() {
    let envs = copy_environment(47300, "Guest", Some(2), None);
    assert_eq!(value(&envs, "NIER_COOP_NEW_GAME_SLOT"), None);
    assert_eq!(value(&envs, "NIER_COOP_NEW_GAME_NAME"), None);
}
