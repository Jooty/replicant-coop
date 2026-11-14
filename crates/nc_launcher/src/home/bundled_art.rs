const BUNDLED_BANNERS: [(&str, &[u8]); 0] = [];
const BUNDLED_PORTRAITS: [(&str, &str, &[u8]); 0] = [];

/// A banner image built into the launcher for this game, from `assets/art/<game>-banner.*`. None is bundled yet;
/// an entry here wins over the Steam library cache.
pub fn bundled_banner(game_id: &str) -> Option<&'static [u8]> {
    BUNDLED_BANNERS
        .iter()
        .find(|(id, _)| *id == game_id)
        .map(|(_, bytes)| *bytes)
}

/// An image of one character built into the launcher, from `assets/art/<game>-<character>.*`. None is bundled
/// yet; an entry here wins over the seat's crop of the key art.
pub fn bundled_portrait(game_id: &str, character_id: &str) -> Option<&'static [u8]> {
    BUNDLED_PORTRAITS
        .iter()
        .find(|(game, character, _)| *game == game_id && *character == character_id)
        .map(|(_, _, bytes)| *bytes)
}
