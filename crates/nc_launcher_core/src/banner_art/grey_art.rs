const GREY_BRIGHTNESS: f32 = 0.5;
const GREY_CONTRAST: f32 = 0.9;

/// The grey level a pixel of a missing game's art is drawn at: its luminance, halved, then a little flatter.
pub fn grey_dimmed(red: u8, green: u8, blue: u8) -> u8 {
    let luminance =
        (0.2126 * f32::from(red) + 0.7152 * f32::from(green) + 0.0722 * f32::from(blue)) / 255.0;
    let dimmed = luminance * GREY_BRIGHTNESS;
    let flattened = (dimmed - 0.5) * GREY_CONTRAST + 0.5;
    (flattened.clamp(0.0, 1.0) * 255.0).round() as u8
}
