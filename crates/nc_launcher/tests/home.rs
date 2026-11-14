use std::path::{Path, PathBuf};

use nc_launcher::home::{CardTone, GameCard, card_view, folder_display};
use nc_launcher::install::ModStatus;
use nc_launcher::steam::GameInstall;
use nc_launcher_core::{Character, GameProfile, GameSupport};
use replicant_game::build::{PE_HEADER_READ_LEN, PeIdentity, SUPPORTED_PE_IDENTITIES};

use crate::scratch::fresh_dir;

const MADE_UP_CHARACTERS: [Character; 1] = [Character {
    id: "only",
    label: "Only",
}];

const SUPPORTED_GAME: GameProfile = GameProfile {
    game_id: "made-up",
    display_name: "Made Up",
    steam_app_id: "424242",
    install_folder_name: "MadeUp",
    exe_names: &["MadeUp.exe"],
    supported_exe_builds: &SUPPORTED_PE_IDENTITIES,
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
        saves: true,
    },
};

const UNSUPPORTED_GAME: GameProfile = GameProfile {
    support: GameSupport {
        install: false,
        start: false,
        saves: false,
    },
    ..SUPPORTED_GAME
};

fn card(
    profile: &'static GameProfile,
    install: Option<GameInstall>,
    status: ModStatus,
) -> GameCard {
    GameCard {
        profile,
        install,
        dll_source: None,
        mod_status: status,
        installed_build: Some("abcd1234".to_owned()),
        ready_build: Some("ef567890".to_owned()),
        exe_supported: true,
    }
}

fn exe_header(identity: PeIdentity) -> Vec<u8> {
    let mut header = vec![0_u8; PE_HEADER_READ_LEN];
    header[..2].copy_from_slice(b"MZ");
    header[0x3C..0x40].copy_from_slice(&0x40_u32.to_le_bytes());
    header[0x40..0x44].copy_from_slice(b"PE\0\0");
    header[0x48..0x4C].copy_from_slice(&identity.time_date_stamp.to_le_bytes());
    header[0x90..0x94].copy_from_slice(&identity.size_of_image.to_le_bytes());
    header
}

fn install_with_exe(profile: &'static GameProfile, name: &str, with_exe: bool) -> GameInstall {
    let game_dir = fresh_dir(name);
    if with_exe {
        std::fs::write(game_dir.join(profile.exe_names[0]), b"x").unwrap();
    }
    GameInstall {
        profile,
        steamapps_dir: PathBuf::from("/lib/steamapps"),
        game_dir,
        proton_prefix: None,
    }
}

#[test]
fn a_game_that_is_not_found_is_greyed_out_and_cannot_be_played() {
    let view = card_view(&card(&SUPPORTED_GAME, None, ModStatus::NotInstalled), None);
    assert_eq!(view.found_text, "Not found");
    assert_eq!(view.found_tone, CardTone::Bad);
    assert!(view.greyed_out);
    assert!(!view.play_enabled);
    assert!(!view.offers_install);
}

#[test]
fn a_folder_without_the_exe_counts_as_not_found() {
    let install = install_with_exe(&SUPPORTED_GAME, "home_no_exe", false);
    let view = card_view(
        &card(&SUPPORTED_GAME, Some(install), ModStatus::NotInstalled),
        None,
    );
    assert!(view.found_text.starts_with("No game executable in"));
    assert!(view.greyed_out && !view.play_enabled);
}

#[test]
fn a_found_game_shows_where_and_can_be_played() {
    let install = install_with_exe(&SUPPORTED_GAME, "home_found", true);
    let where_text = install.game_dir.display().to_string();
    let view = card_view(
        &card(&SUPPORTED_GAME, Some(install), ModStatus::NotInstalled),
        None,
    );
    assert_eq!(view.title, "Made Up");
    assert_eq!(view.found_text, where_text);
    assert_eq!(view.found_tone, CardTone::Good);
    assert!(!view.greyed_out && view.play_enabled && view.offers_install);
    assert_eq!(view.mod_text, "Not installed");
    assert_eq!(view.mod_action, Some("Install the mod"));
}

#[test]
fn an_exe_of_another_build_turns_the_folder_red_but_still_plays() {
    let install = install_with_exe(&SUPPORTED_GAME, "home_other_build", true);
    let mut other = card(&SUPPORTED_GAME, Some(install), ModStatus::Current);
    other.exe_supported = false;
    let view = card_view(&other, None);
    assert_eq!(view.found_tone, CardTone::Bad);
    assert!(view.play_enabled);
}

#[test]
fn the_exe_build_is_read_from_the_head_of_the_exe() {
    let install = install_with_exe(&SUPPORTED_GAME, "home_exe_build", false);
    let exe = install.game_dir.join(SUPPORTED_GAME.exe_names[0]);
    std::fs::write(&exe, exe_header(SUPPORTED_PE_IDENTITIES[0])).unwrap();
    assert!(install.exe_build_supported());
    let other = PeIdentity {
        size_of_image: SUPPORTED_PE_IDENTITIES[0].size_of_image + 0x1000,
        ..SUPPORTED_PE_IDENTITIES[0]
    };
    std::fs::write(&exe, exe_header(other)).unwrap();
    assert!(!install.exe_build_supported());
    std::fs::write(&exe, b"x").unwrap();
    assert!(!install.exe_build_supported());
}

#[test]
fn the_mod_line_follows_the_install_state_and_names_the_build() {
    let install = install_with_exe(&SUPPORTED_GAME, "home_mod_states", true);
    let line =
        |status| card_view(&card(&SUPPORTED_GAME, Some(install.clone()), status), None).mod_text;
    assert_eq!(line(ModStatus::Current), "Installed, build abcd1234");
    assert_eq!(line(ModStatus::Outdated), "Installed, build abcd1234");
    assert_eq!(
        line(ModStatus::Foreign),
        "A different loader.dll is in the game folder"
    );
}

#[test]
fn a_profile_that_supports_nothing_says_the_mod_is_not_built_and_offers_no_install() {
    let install = install_with_exe(&UNSUPPORTED_GAME, "home_unsupported", true);
    let view = card_view(
        &card(&UNSUPPORTED_GAME, Some(install), ModStatus::NotInstalled),
        None,
    );
    assert_eq!(view.mod_text, "Not built yet");
    assert_eq!(view.mod_tone, CardTone::Muted);
    assert!(!view.offers_install);
    assert_eq!(view.mod_action, None);
    assert!(view.play_enabled);
}

#[test]
fn an_older_build_offers_an_update_and_a_current_one_does_not() {
    let install = install_with_exe(&SUPPORTED_GAME, "home_update_offer", true);
    let offered = |status| {
        card_view(&card(&SUPPORTED_GAME, Some(install.clone()), status), None)
            .update_text
            .is_some()
    };
    assert!(offered(ModStatus::Outdated));
    assert!(!offered(ModStatus::Current));
    assert!(!offered(ModStatus::Foreign));
}

#[test]
fn the_update_line_is_the_update_control() {
    let install = install_with_exe(&SUPPORTED_GAME, "home_update_line", true);
    let outdated = card_view(
        &card(&SUPPORTED_GAME, Some(install.clone()), ModStatus::Outdated),
        None,
    );
    assert!(outdated.update_text.is_some());
    assert_eq!(outdated.mod_action, None, "no separate update button");
    assert!(!outdated.update_from_server);
    let from_server = card_view(
        &card(&SUPPORTED_GAME, Some(install), ModStatus::Current),
        Some("7b04de2"),
    );
    assert_eq!(from_server.mod_action, None, "no separate update button");
    assert!(from_server.update_from_server);
}

#[test]
fn a_game_that_is_not_found_offers_no_mod_action() {
    let view = card_view(&card(&SUPPORTED_GAME, None, ModStatus::NotInstalled), None);
    assert_eq!(view.mod_action, None);
}

#[test]
fn an_outdated_mod_names_the_ready_build_on_the_update_line() {
    let install = install_with_exe(&SUPPORTED_GAME, "home_ready_build", true);
    let view = card_view(
        &card(&SUPPORTED_GAME, Some(install), ModStatus::Outdated),
        None,
    );
    assert_eq!(view.update_text.as_deref(), Some("update ef567890 ready"));
    assert!(!view.update_from_server);
}

#[test]
fn a_build_on_the_update_server_is_offered_for_an_installed_mod_only() {
    let install = install_with_exe(&SUPPORTED_GAME, "home_server_build", true);
    let view = |status| {
        card_view(
            &card(&SUPPORTED_GAME, Some(install.clone()), status),
            Some("7b04de2"),
        )
    };
    let current = view(ModStatus::Current);
    assert_eq!(current.update_text.as_deref(), Some("update 7b04de2 ready"));
    assert_eq!(current.mod_action, None);
    assert!(current.update_from_server);
    assert_eq!(view(ModStatus::NotInstalled).update_text, None);
    assert_eq!(view(ModStatus::Foreign).update_text, None);
}

#[test]
fn nothing_offers_an_update_when_no_newer_build_exists() {
    let install = install_with_exe(&SUPPORTED_GAME, "home_no_update", true);
    let view = card_view(
        &card(&SUPPORTED_GAME, Some(install), ModStatus::Current),
        None,
    );
    assert_eq!(view.update_text, None);
    assert_eq!(view.mod_action, None);
}

#[test]
fn the_folder_is_shown_from_home_on_one_line() {
    let shown = folder_display(
        Path::new("/home/jay/.local/share/Steam/steamapps/common/NieR Replicant ver.1.22474487139"),
        Some(Path::new("/home/jay")),
    );
    assert_eq!(
        shown,
        "~/.local/share/Steam/steamapps/common/NieR Replicant ver.1.22474487139"
    );
    let elsewhere = folder_display(
        Path::new("/games/NieR Replicant ver.1.22474487139"),
        Some(Path::new("/home/jay")),
    );
    assert_eq!(elsewhere, "/games/NieR Replicant ver.1.22474487139");
}
