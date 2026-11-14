use std::path::{Path, PathBuf};

const APP_CACHE_FOLDER: &str = "appcache";
const LIBRARY_CACHE_FOLDER: &str = "librarycache";
const HERO_FILE_NAME: &str = "library_hero.jpg";

/// Where a game's banner art comes from, best first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BannerArtSource {
    /// An image built into the launcher.
    Bundled(&'static [u8]),
    /// The hero image in the player's own Steam library cache.
    File(PathBuf),
    /// No art: the banner draws its own shapes.
    Shapes,
}

/// A bundled image wins; then the first Steam root whose library cache holds the game's hero image.
pub fn find_banner_art(
    bundled: Option<&'static [u8]>,
    steam_roots: &[PathBuf],
    steam_app_id: &str,
) -> BannerArtSource {
    if let Some(bytes) = bundled {
        return BannerArtSource::Bundled(bytes);
    }
    steam_roots
        .iter()
        .flat_map(|root| library_cache_hero_paths(root, steam_app_id))
        .find(|path| path.is_file())
        .map_or(BannerArtSource::Shapes, BannerArtSource::File)
}

/// The hero image's path in today's cache layout, then in the flat layout older Steam clients used.
pub fn library_cache_hero_paths(steam_root: &Path, steam_app_id: &str) -> [PathBuf; 2] {
    let cache = steam_root.join(APP_CACHE_FOLDER).join(LIBRARY_CACHE_FOLDER);
    [
        cache.join(steam_app_id).join(HERO_FILE_NAME),
        cache.join(format!("{steam_app_id}_{HERO_FILE_NAME}")),
    ]
}
