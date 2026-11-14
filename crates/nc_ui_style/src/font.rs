mod bytes;
mod sans_faces;

pub use bytes::font_bytes;
pub use sans_faces::{SANS_LIGHT_FONT_BYTES, SANS_MEDIUM_FONT_BYTES, SANS_REGULAR_FONT_BYTES};

pub const FONT_GLYPH_RANGES: [u32; 9] = [
    0x0020, 0x00FF, 0x0100, 0x024F, 0x0370, 0x03FF, 0x0400, 0x04FF, 0,
];
