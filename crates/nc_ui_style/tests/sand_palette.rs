use nc_ui_style::{
    BAND, GROUND, HAIRLINE, HAIRLINE_STRONG, HARD_SHADOW, HATCH_LINE, HOVER_WASH, INK, INK_MUTED,
    INK_ON_SAND, INK_UNAVAILABLE, NOT_READY, READY, SAND, SANS_MEDIUM_FONT_BYTES,
    SANS_REGULAR_FONT_BYTES,
};

#[test]
fn the_launcher_ground_and_ink_are_unchanged() {
    assert_eq!(GROUND, [0x1f, 0x1d, 0x18, 255]);
    assert_eq!(INK, [0xd8, 0xd0, 0xb7, 255]);
    assert_eq!(READY, [0x9e, 0xc2, 0x7f, 255]);
    assert_eq!(NOT_READY, [0xdf, 0x8e, 0x73, 255]);
}

#[test]
fn ink_on_sand_is_the_ground_and_sand_is_the_ink() {
    assert_eq!(INK_ON_SAND, GROUND);
    assert_eq!(SAND, INK);
}

#[test]
fn every_faint_line_is_the_ink_made_transparent() {
    for faint in [HAIRLINE, HAIRLINE_STRONG, HATCH_LINE, HOVER_WASH] {
        assert_eq!(faint[..3], INK[..3]);
        assert!(faint[3] < 255);
    }
}

#[test]
fn inks_fade_from_full_to_unavailable() {
    for channel in 0..3 {
        assert!(INK[channel] > INK_MUTED[channel]);
        assert!(INK_MUTED[channel] > INK_UNAVAILABLE[channel]);
        assert!(INK_UNAVAILABLE[channel] > BAND[channel]);
    }
    assert!(HARD_SHADOW[0] > GROUND[0]);
}

#[test]
fn the_shared_sans_faces_are_opentype_files() {
    for face in [SANS_REGULAR_FONT_BYTES, SANS_MEDIUM_FONT_BYTES] {
        assert!(face.starts_with(b"OTTO"));
    }
}
