use std::path::{Path, PathBuf};

use nc_launcher::steam::{
    candidate_steam_roots, find_game, install_from_game_dir, parse_reg_query_value,
};

use nc_launcher_core::{Character, GameProfile, GameSupport};

use crate::scratch::fresh_dir;

const GAME_FOLDER: &str = "Some Game Folder";

const MADE_UP_CHARACTERS: [Character; 1] = [Character {
    id: "only",
    label: "Only",
}];

const MADE_UP_GAME: GameProfile = GameProfile {
    game_id: "made-up",
    display_name: "Made Up",
    steam_app_id: "424242",
    install_folder_name: GAME_FOLDER,
    exe_names: &["MadeUp.exe", "MadeUpCompat.exe"],
    supported_exe_builds: &[],
    loader_dll_name: "loader.dll",
    hook_dll_name: "made_up_hook.dll",
    link_file_name: "made-up.toml",
    characters: &MADE_UP_CHARACTERS,
    characters_shareable: false,
    character_art: &[],
    modes: &[nc_launcher_core::CO_OP_CAMPAIGN],
    support: GameSupport {
        install: true,
        start: true,
        saves: false,
    },
};

const OTHER_GAME: GameProfile = GameProfile {
    game_id: "other",
    steam_app_id: "515151",
    ..MADE_UP_GAME
};

fn write_manifest(profile: &GameProfile, library: &Path) {
    let steamapps = library.join("steamapps");
    std::fs::create_dir_all(steamapps.join("common").join(GAME_FOLDER)).unwrap();
    std::fs::write(
        steamapps.join(format!("appmanifest_{}.acf", profile.steam_app_id)),
        format!(
            "\"AppState\"\n{{\n\t\"appid\"\t\"{}\"\n\t\"installdir\"\t\"{GAME_FOLDER}\"\n}}\n",
            profile.steam_app_id
        ),
    )
    .unwrap();
}

fn write_library_list(steam_root: &Path, libraries: &[&Path]) {
    let steamapps = steam_root.join("steamapps");
    std::fs::create_dir_all(&steamapps).unwrap();
    let mut text = String::from("\"libraryfolders\"\n{\n");
    for (index, library) in libraries.iter().enumerate() {
        text.push_str(&format!(
            "\t\"{index}\"\n\t{{\n\t\t\"path\"\t\"{}\"\n\t}}\n",
            library.display()
        ));
    }
    text.push_str("}\n");
    std::fs::write(steamapps.join("libraryfolders.vdf"), text).unwrap();
}

#[test]
fn game_in_a_second_library_is_found_through_the_library_list() {
    let base = fresh_dir("second_library");
    let steam_root = base.join("Steam");
    let other = base.join("Other");
    write_library_list(&steam_root, &[&steam_root, &other]);
    write_manifest(&MADE_UP_GAME, &other);
    let found = find_game(&MADE_UP_GAME, &[steam_root]).expect("game is found");
    assert_eq!(
        found.game_dir,
        other.join("steamapps/common").join(GAME_FOLDER)
    );
    assert_eq!(found.proton_prefix, None);
}

#[test]
fn proton_prefix_is_reported_only_when_it_exists() {
    let base = fresh_dir("proton_prefix");
    let steam_root = base.join("Steam");
    write_library_list(&steam_root, &[&steam_root]);
    write_manifest(&MADE_UP_GAME, &steam_root);
    let prefix = steam_root
        .join("steamapps/compatdata")
        .join(MADE_UP_GAME.steam_app_id)
        .join("pfx");
    std::fs::create_dir_all(&prefix).unwrap();
    let found = find_game(&MADE_UP_GAME, &[steam_root]).expect("game is found");
    assert_eq!(found.proton_prefix, Some(prefix));
}

#[test]
fn missing_manifest_means_not_found() {
    let base = fresh_dir("no_manifest");
    let steam_root = base.join("Steam");
    write_library_list(&steam_root, &[&steam_root]);
    assert_eq!(find_game(&MADE_UP_GAME, &[steam_root]), None);
}

#[test]
fn a_manifest_without_its_game_folder_means_not_found() {
    let base = fresh_dir("no_game_folder");
    let steam_root = base.join("Steam");
    write_manifest(&MADE_UP_GAME, &steam_root);
    std::fs::remove_dir_all(steam_root.join("steamapps/common").join(GAME_FOLDER)).unwrap();
    assert_eq!(find_game(&MADE_UP_GAME, &[steam_root]), None);
}

#[test]
fn manual_game_folder_gets_its_steamapps_folder_derived() {
    let game_dir = PathBuf::from("/lib/steamapps/common/Game");
    let install = install_from_game_dir(&MADE_UP_GAME, &game_dir);
    assert_eq!(install.steamapps_dir, PathBuf::from("/lib/steamapps"));
    assert_eq!(install.game_dir, game_dir);
}

#[test]
fn linux_candidates_cover_native_flatpak_and_snap() {
    let roots = candidate_steam_roots(Some(Path::new("/home/u")), None);
    let has = |suffix: &str| roots.iter().any(|root| root.ends_with(suffix));
    assert!(has(".steam/steam"));
    assert!(has(".local/share/Steam"));
    assert!(has("com.valvesoftware.Steam/.local/share/Steam"));
    assert!(has("snap/steam/common/.local/share/Steam"));
}

#[test]
fn registry_path_comes_first_among_candidates() {
    let roots = candidate_steam_roots(Some(Path::new("/home/u")), Some("C:\\Steam"));
    assert_eq!(roots.first(), Some(&PathBuf::from("C:\\Steam")));
}

#[test]
fn reg_query_output_gives_the_steam_path() {
    let output = "\r\nHKEY_CURRENT_USER\\Software\\Valve\\Steam\r\n    SteamPath    REG_SZ    c:/program files (x86)/steam\r\n\r\n";
    assert_eq!(
        parse_reg_query_value(output).as_deref(),
        Some("c:/program files (x86)/steam")
    );
    assert_eq!(parse_reg_query_value("ERROR: not found"), None);
}

#[test]
fn a_library_holding_another_game_is_not_mistaken_for_this_one() {
    let base = fresh_dir("other_game_only");
    let steam_root = base.join("Steam");
    write_library_list(&steam_root, &[&steam_root]);
    write_manifest(&OTHER_GAME, &steam_root);
    assert_eq!(
        find_game(&MADE_UP_GAME, std::slice::from_ref(&steam_root)),
        None
    );
    let found = find_game(&OTHER_GAME, &[steam_root]).expect("the other game is found");
    assert_eq!(found.profile.game_id, "other");
}

#[test]
fn two_games_in_one_library_are_each_found_with_their_own_compat_folder() {
    let base = fresh_dir("two_games");
    let steam_root = base.join("Steam");
    write_library_list(&steam_root, &[&steam_root]);
    write_manifest(&MADE_UP_GAME, &steam_root);
    write_manifest(&OTHER_GAME, &steam_root);
    let first = find_game(&MADE_UP_GAME, std::slice::from_ref(&steam_root)).unwrap();
    let second = find_game(&OTHER_GAME, &[steam_root]).unwrap();
    assert!(first.proton_compat_dir().ends_with("compatdata/424242"));
    assert!(second.proton_compat_dir().ends_with("compatdata/515151"));
}

#[test]
fn a_game_with_two_exes_is_present_when_either_exe_is_there() {
    let base = fresh_dir("two_exes");
    let steam_root = base.join("Steam");
    write_library_list(&steam_root, &[&steam_root]);
    write_manifest(&MADE_UP_GAME, &steam_root);
    let found = find_game(&MADE_UP_GAME, &[steam_root]).unwrap();
    assert!(!found.exe_present());
    assert_eq!(found.exe_path(), found.game_dir.join("MadeUp.exe"));
    std::fs::write(found.game_dir.join("MadeUpCompat.exe"), b"x").unwrap();
    assert!(found.exe_present());
    assert_eq!(found.exe_path(), found.game_dir.join("MadeUpCompat.exe"));
    std::fs::write(found.game_dir.join("MadeUp.exe"), b"x").unwrap();
    assert_eq!(found.exe_path(), found.game_dir.join("MadeUp.exe"));
}
