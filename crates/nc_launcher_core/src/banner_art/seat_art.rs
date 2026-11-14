use crate::banner_art::ArtFocus;
use crate::profile::GameProfile;

/// How a lobby seat crops its picture: the point kept at the seat's centre, how much of the picture's height
/// the seat shows, and whether it is mirrored.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeatArt {
    pub focus: ArtFocus,
    pub height_share: f32,
    pub mirrored: bool,
}

/// What a seat shows: the image bundled for its character, or the game's key art cropped for that character.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SeatPicture {
    Portrait {
        character_id: &'static str,
        crop: SeatArt,
    },
    KeyArt(SeatArt),
}

const WHOLE_PICTURE: SeatArt = SeatArt {
    focus: ArtFocus { x: 0.5, y: 0.5 },
    height_share: 1.0,
    mirrored: false,
};

impl SeatPicture {
    pub fn crop(&self) -> SeatArt {
        match self {
            SeatPicture::Portrait { crop, .. } | SeatPicture::KeyArt(crop) => *crop,
        }
    }

    fn mirror(&mut self) {
        match self {
            SeatPicture::Portrait { crop, .. } | SeatPicture::KeyArt(crop) => crop.mirrored = true,
        }
    }
}

/// The picture of each seat, from the character on it (`None` for an open seat). A character with a bundled
/// image (`has_portrait`) shows it whole; any other shows the key art around its own focus. A seat whose
/// picture would be identical to an earlier, unmirrored seat's is mirrored.
pub fn seat_pictures(
    profile: &GameProfile,
    characters: &[Option<&str>],
    has_portrait: impl Fn(&str) -> bool,
) -> Vec<SeatPicture> {
    let mut pictures: Vec<SeatPicture> = Vec::with_capacity(characters.len());
    for character in characters {
        let mut picture = unmirrored_picture(profile, *character, &has_portrait);
        if pictures.contains(&picture) {
            picture.mirror();
        }
        pictures.push(picture);
    }
    pictures
}

fn unmirrored_picture(
    profile: &GameProfile,
    character: Option<&str>,
    has_portrait: &impl Fn(&str) -> bool,
) -> SeatPicture {
    let known = character.and_then(|id| profile.character(id));
    if let Some(known) = known
        && has_portrait(known.id)
    {
        return SeatPicture::Portrait {
            character_id: known.id,
            crop: WHOLE_PICTURE,
        };
    }
    let crop = known
        .and_then(|known| profile.character_art(known.id))
        .map_or(WHOLE_PICTURE, |art| SeatArt {
            focus: art.focus(),
            height_share: art.height_share(),
            mirrored: false,
        });
    SeatPicture::KeyArt(crop)
}
