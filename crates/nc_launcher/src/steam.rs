mod detect;
mod discovery;
mod library;
mod vdf;

pub use detect::{detect_game, hide_console_window, read_registry_steam_path, steam_roots};
pub use discovery::{
    GameInstall, candidate_steam_roots, find_game, install_from_game_dir, parse_reg_query_value,
};
pub use library::{library_paths, manifest_install_dir};
pub use vdf::{VdfError, VdfNode, parse_vdf};
