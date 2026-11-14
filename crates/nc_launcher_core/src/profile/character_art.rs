use crate::banner_art::ArtFocus;

const PER_MILLE: f32 = 1000.0;

/// Where a lobby seat crops the game's key art for one character, until an image of that character is
/// bundled: the point kept at the seat's centre and how much of the art's height the seat shows, each in
/// thousandths of the art's width or height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterArt {
    pub character_id: &'static str,
    pub focus_x_per_mille: u16,
    pub focus_y_per_mille: u16,
    pub height_share_per_mille: u16,
}

impl CharacterArt {
    pub fn focus(&self) -> ArtFocus {
        ArtFocus {
            x: f32::from(self.focus_x_per_mille) / PER_MILLE,
            y: f32::from(self.focus_y_per_mille) / PER_MILLE,
        }
    }

    pub fn height_share(&self) -> f32 {
        f32::from(self.height_share_per_mille) / PER_MILLE
    }
}
