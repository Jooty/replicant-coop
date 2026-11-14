use replicant_game::build::SUPPORTED_PE_IDENTITIES;

use crate::profile::{CO_OP_CAMPAIGN, Character, CharacterArt, GameMode, GameProfile, GameSupport};

const REPLICANT_CHARACTERS: [Character; 2] = [
    Character {
        id: "nier",
        label: "Nier",
    },
    Character {
        id: "kaine",
        label: "Kainé",
    },
];

const REPLICANT_CHARACTER_ART: [CharacterArt; 2] = [
    CharacterArt {
        character_id: "nier",
        focus_x_per_mille: 503,
        focus_y_per_mille: 400,
        height_share_per_mille: 800,
    },
    CharacterArt {
        character_id: "kaine",
        focus_x_per_mille: 610,
        focus_y_per_mille: 640,
        height_share_per_mille: 620,
    },
];

const REPLICANT_MODES: [GameMode; 1] = [CO_OP_CAMPAIGN];

pub const REPLICANT: GameProfile = GameProfile {
    game_id: "replicant",
    display_name: "NieR Replicant",
    steam_app_id: "1113560",
    install_folder_name: "NieR Replicant ver.1.22474487139",
    exe_names: &["NieR Replicant ver.1.22474487139.exe"],
    supported_exe_builds: &SUPPORTED_PE_IDENTITIES,
    loader_dll_name: "xinput9_1_0.dll",
    hook_dll_name: "replicant_hook.dll",
    link_file_name: "nier-coop.toml",
    characters: &REPLICANT_CHARACTERS,
    characters_shareable: true,
    character_art: &REPLICANT_CHARACTER_ART,
    modes: &REPLICANT_MODES,
    support: GameSupport {
        install: true,
        start: true,
        saves: true,
    },
};
