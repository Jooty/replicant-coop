use crate::profile::REPLICANT;

const REPLICANT_BANNER_TAGS: [&str; 2] = ["Co-op campaign", "2 players"];

/// A point in the art, as fractions of its width and height, that a crop keeps in frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArtFocus {
    pub x: f32,
    pub y: f32,
}

/// What a game's home banner shows besides the install facts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BannerFacts {
    pub tags: &'static [&'static str],
    pub focus: ArtFocus,
}

const CENTRE_FOCUS: ArtFocus = ArtFocus { x: 0.5, y: 0.5 };
const REPLICANT_ART_FOCUS: ArtFocus = ArtFocus { x: 0.51, y: 0.45 };

pub fn banner_facts(game_id: &str) -> BannerFacts {
    if game_id == REPLICANT.game_id {
        BannerFacts {
            tags: &REPLICANT_BANNER_TAGS,
            focus: REPLICANT_ART_FOCUS,
        }
    } else {
        BannerFacts {
            tags: &[],
            focus: CENTRE_FOCUS,
        }
    }
}
