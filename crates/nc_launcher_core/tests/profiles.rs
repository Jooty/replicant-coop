use nc_launcher_core::{
    ALL_PROFILES, Character, GameProfile, GameSupport, REPLICANT, character_allowed,
    character_choices, profile_by_id,
};
use replicant_game::build::{PE_HEADER_READ_LEN, PeIdentity, SUPPORTED_PE_IDENTITY};

const MADE_UP_CHARACTERS: [Character; 3] = [
    Character {
        id: "red",
        label: "Red",
    },
    Character {
        id: "green",
        label: "Green",
    },
    Character {
        id: "blue",
        label: "Blue",
    },
];

const SOLO_ONLY_SUPPORT: GameSupport = GameSupport {
    install: false,
    start: false,
    saves: false,
};

const MADE_UP_UNIQUE_CHARACTERS: GameProfile = GameProfile {
    game_id: "made-up-unique",
    display_name: "Made Up",
    steam_app_id: "1",
    install_folder_name: "MadeUp",
    exe_names: &["MadeUp.exe", "MadeUpCompat.exe"],
    supported_exe_builds: &[],
    loader_dll_name: "loader.dll",
    hook_dll_name: "made_up_hook.dll",
    link_file_name: "made-up.toml",
    characters: &MADE_UP_CHARACTERS,
    characters_shareable: false,
    character_art: &[],
    modes: &[nc_launcher_core::CO_OP_CAMPAIGN],
    support: SOLO_ONLY_SUPPORT,
};

const MADE_UP_SHARED_CHARACTERS: GameProfile = GameProfile {
    characters_shareable: true,
    character_art: &[],
    modes: &[nc_launcher_core::CO_OP_CAMPAIGN],
    ..MADE_UP_UNIQUE_CHARACTERS
};

fn ids(choices: &[&Character]) -> Vec<&'static str> {
    choices.iter().map(|character| character.id).collect()
}

#[test]
fn two_players_cannot_hold_the_same_character_when_the_game_forbids_it() {
    assert!(!character_allowed(
        &MADE_UP_UNIQUE_CHARACTERS,
        "red",
        &["red"]
    ));
    assert!(character_allowed(
        &MADE_UP_UNIQUE_CHARACTERS,
        "green",
        &["red"]
    ));
    assert_eq!(
        ids(&character_choices(&MADE_UP_UNIQUE_CHARACTERS, &["red"])),
        ["green", "blue"]
    );
}

#[test]
fn two_players_may_share_a_character_when_the_game_allows_it() {
    assert!(character_allowed(
        &MADE_UP_SHARED_CHARACTERS,
        "red",
        &["red"]
    ));
    assert_eq!(
        ids(&character_choices(&MADE_UP_SHARED_CHARACTERS, &["red"])),
        ["red", "green", "blue"]
    );
}

#[test]
fn a_character_the_game_does_not_list_is_never_allowed() {
    assert!(!character_allowed(
        &MADE_UP_SHARED_CHARACTERS,
        "purple",
        &[]
    ));
}

#[test]
fn a_profile_names_its_loader_stem_and_its_exes() {
    assert_eq!(MADE_UP_UNIQUE_CHARACTERS.loader_dll_stem(), "loader");
    assert!(MADE_UP_UNIQUE_CHARACTERS.is_game_exe("MadeUpCompat.exe"));
    assert!(!MADE_UP_UNIQUE_CHARACTERS.is_game_exe("Other.exe"));
    assert!(!MADE_UP_UNIQUE_CHARACTERS.supports_anything());
}

#[test]
fn replicant_keeps_today_values_and_is_listed() {
    assert_eq!(REPLICANT.steam_app_id, "1113560");
    assert_eq!(REPLICANT.loader_dll_name, "xinput9_1_0.dll");
    assert_eq!(REPLICANT.loader_dll_stem(), "xinput9_1_0");
    let listed = profile_by_id("replicant").expect("replicant is listed");
    assert!(listed.characters_shareable);
    assert!(listed.support.install && listed.support.start && listed.support.saves);
    assert_eq!(listed, &REPLICANT);
    assert!(ALL_PROFILES.contains(&&REPLICANT));
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

#[test]
fn only_the_supported_exe_build_is_supported() {
    assert!(REPLICANT.supports_exe_header(&exe_header(SUPPORTED_PE_IDENTITY)));
    let other = PeIdentity {
        time_date_stamp: SUPPORTED_PE_IDENTITY.time_date_stamp + 1,
        ..SUPPORTED_PE_IDENTITY
    };
    assert!(!REPLICANT.supports_exe_header(&exe_header(other)));
    assert!(!REPLICANT.supports_exe_header(b""));
    assert!(!MADE_UP_UNIQUE_CHARACTERS.supports_exe_header(&exe_header(SUPPORTED_PE_IDENTITY)));
}
