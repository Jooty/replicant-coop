use crate::banner_art::ArtFocus;

/// The part of an image to show, in texture coordinates from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArtCrop {
    pub min_u: f32,
    pub min_v: f32,
    pub max_u: f32,
    pub max_v: f32,
}

/// Scales the image to cover the area and places its focus at `anchor` (fractions of the area), as near as
/// the image's edges allow.
pub fn crop_around_focus(
    image_size: (f32, f32),
    area_size: (f32, f32),
    focus: ArtFocus,
    anchor: (f32, f32),
) -> ArtCrop {
    let (image_width, image_height) = image_size;
    let (area_width, area_height) = area_size;
    if image_width <= 0.0 || image_height <= 0.0 || area_width <= 0.0 || area_height <= 0.0 {
        return ArtCrop {
            min_u: 0.0,
            min_v: 0.0,
            max_u: 1.0,
            max_v: 1.0,
        };
    }
    let scale = (area_width / image_width).max(area_height / image_height);
    let shown_u = (area_width / scale / image_width).min(1.0);
    let shown_v = (area_height / scale / image_height).min(1.0);
    let min_u = (focus.x - anchor.0 * shown_u).clamp(0.0, 1.0 - shown_u);
    let min_v = (focus.y - anchor.1 * shown_v).clamp(0.0, 1.0 - shown_v);
    ArtCrop {
        min_u,
        min_v,
        max_u: min_u + shown_u,
        max_v: min_v + shown_v,
    }
}

/// Shows `height_share` of the image's height in the area, centred on `focus` as near as the image's edges
/// allow; the width follows the area's shape.
pub fn crop_share_around_focus(
    image_size: (f32, f32),
    area_size: (f32, f32),
    focus: ArtFocus,
    height_share: f32,
) -> ArtCrop {
    let (image_width, image_height) = image_size;
    let (area_width, area_height) = area_size;
    let cover = crop_around_focus(image_size, area_size, focus, (0.5, 0.5));
    if image_width <= 0.0 || image_height <= 0.0 || area_width <= 0.0 || area_height <= 0.0 {
        return cover;
    }
    let shown_v = height_share.clamp(0.0, 1.0).min(cover.max_v - cover.min_v);
    let shown_u = (shown_v * image_height * area_width / area_height / image_width).min(1.0);
    let shown_v = shown_u * image_width * area_height / area_width / image_height;
    let min_u = (focus.x - shown_u / 2.0).clamp(0.0, 1.0 - shown_u);
    let min_v = (focus.y - shown_v / 2.0).clamp(0.0, 1.0 - shown_v);
    ArtCrop {
        min_u,
        min_v,
        max_u: min_u + shown_u,
        max_v: min_v + shown_v,
    }
}
