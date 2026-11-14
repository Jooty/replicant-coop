use std::path::{Path, PathBuf};

use nc_launcher_core::{
    ArtFocus, BannerArtSource, REPLICANT, SeatPicture, banner_facts, crop_around_focus,
    crop_share_around_focus, fade_alpha, fade_stops, find_banner_art, library_cache_hero_paths,
    seat_pictures,
};

const MADE_UP_APP_ID: &str = "424242";
const BUNDLED_BYTES: &[u8] = b"bundled";
const HERO_SIZE_PX: (f32, f32) = (1920.0, 620.0);

fn scratch_root(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("test-scratch/banner-art")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch folder");
    dir
}

fn put_file(path: &Path) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("cache folder");
    std::fs::write(path, b"jpeg").expect("hero file");
}

#[test]
fn art_is_found_in_the_steam_library_cache() {
    let empty_root = scratch_root("found-empty");
    let steam_root = scratch_root("found-steam");
    let hero = steam_root.join("appcache/librarycache/424242/library_hero.jpg");
    put_file(&hero);
    let found = find_banner_art(None, &[empty_root, steam_root], MADE_UP_APP_ID);
    assert_eq!(found, BannerArtSource::File(hero));
}

#[test]
fn the_flat_cache_name_of_older_steam_clients_is_found_too() {
    let steam_root = scratch_root("flat");
    let hero = steam_root.join("appcache/librarycache/424242_library_hero.jpg");
    put_file(&hero);
    let found = find_banner_art(None, std::slice::from_ref(&steam_root), MADE_UP_APP_ID);
    assert_eq!(found, BannerArtSource::File(hero));
    assert_eq!(
        library_cache_hero_paths(&steam_root, MADE_UP_APP_ID)[0],
        steam_root.join("appcache/librarycache/424242/library_hero.jpg")
    );
}

#[test]
fn a_bundled_banner_wins_over_the_cache() {
    let steam_root = scratch_root("bundled");
    put_file(&steam_root.join("appcache/librarycache/424242/library_hero.jpg"));
    let found = find_banner_art(Some(BUNDLED_BYTES), &[steam_root], MADE_UP_APP_ID);
    assert_eq!(found, BannerArtSource::Bundled(BUNDLED_BYTES));
}

#[test]
fn a_missing_file_falls_back_to_shapes() {
    let steam_root = scratch_root("missing");
    put_file(&steam_root.join("appcache/librarycache/999/library_hero.jpg"));
    std::fs::create_dir_all(steam_root.join("appcache/librarycache/424242")).expect("empty folder");
    assert_eq!(
        find_banner_art(None, &[steam_root], MADE_UP_APP_ID),
        BannerArtSource::Shapes
    );
    assert_eq!(
        find_banner_art(None, &[], MADE_UP_APP_ID),
        BannerArtSource::Shapes
    );
}

#[test]
fn the_crop_keeps_the_focus_in_frame_as_the_area_narrows() {
    let focus = ArtFocus { x: 0.51, y: 0.45 };
    for width in [760.0, 520.0, 360.0, 220.0] {
        let crop = crop_around_focus(HERO_SIZE_PX, (width, 300.0), focus, (0.5, 0.5));
        assert!(
            crop.min_u <= focus.x && focus.x <= crop.max_u,
            "width {width}"
        );
        assert!(crop.min_u >= 0.0 && crop.max_u <= 1.0);
        let shown_width_px = (crop.max_u - crop.min_u) * HERO_SIZE_PX.0;
        let shown_height_px = (crop.max_v - crop.min_v) * HERO_SIZE_PX.1;
        assert!((shown_width_px / shown_height_px - width / 300.0).abs() < 0.01);
    }
}

#[test]
fn the_crop_stops_at_the_image_edge() {
    let focus = ArtFocus { x: 0.98, y: 0.5 };
    let crop = crop_around_focus(HERO_SIZE_PX, (400.0, 300.0), focus, (0.5, 0.5));
    assert!((crop.max_u - 1.0).abs() < f32::EPSILON);
}

#[test]
fn the_fade_is_opaque_at_the_text_and_eases_out_to_clear() {
    assert!((fade_alpha(0.0) - 1.0).abs() < f32::EPSILON);
    assert!(fade_alpha(1.0).abs() < f32::EPSILON);
    let stops = fade_stops(9);
    assert_eq!(stops.len(), 9);
    assert!(stops.windows(2).all(|pair| pair[1].alpha <= pair[0].alpha));
    let first_step = stops[0].alpha - stops[1].alpha;
    let middle_step = stops[4].alpha - stops[5].alpha;
    assert!(
        first_step < middle_step,
        "the ramp eases in, not a straight line"
    );
}

#[test]
fn the_game_has_its_own_tags_and_focus() {
    let replicant = banner_facts(REPLICANT.game_id);
    let made_up = banner_facts("made-up");
    assert!(replicant.tags.contains(&"2 players"));
    assert_ne!(replicant.focus, made_up.focus);
    assert!(made_up.tags.is_empty());
}

#[test]
fn a_missing_game_art_is_grey_and_never_brighter_than_half() {
    let white = nc_launcher_core::grey_dimmed(255, 255, 255);
    let black = nc_launcher_core::grey_dimmed(0, 0, 0);
    assert!(white <= 128 && black < white);
    let red = nc_launcher_core::grey_dimmed(255, 0, 0);
    assert!(red > black && red < white);
}

const NORMAL_BODY: &str = "nier";
const SWAPPED_BODY: &str = "kaine";

fn no_portraits(_: &str) -> bool {
    false
}

#[test]
fn art_follows_the_character_on_the_seat() {
    let bodies = [NORMAL_BODY, SWAPPED_BODY];
    let host = seat_pictures(&REPLICANT, &[Some(NORMAL_BODY)], no_portraits)[0];
    for guest in bodies {
        let shown = seat_pictures(&REPLICANT, &[Some(NORMAL_BODY), Some(guest)], no_portraits);
        assert_eq!(shown[0], host, "the host's seat keeps its picture");
        let alone = seat_pictures(&REPLICANT, &[Some(guest)], no_portraits)[0];
        assert_eq!(shown[1].crop().focus, alone.crop().focus);
        assert_eq!(shown[1].crop().mirrored, guest == NORMAL_BODY);
    }
    let crops: Vec<_> = bodies
        .iter()
        .map(|body| seat_pictures(&REPLICANT, &[Some(body)], no_portraits)[0].crop())
        .collect();
    for (index, crop) in crops.iter().enumerate() {
        assert!(!crop.mirrored);
        assert!(crops[index + 1..].iter().all(|other| other != crop));
    }
}

#[test]
fn a_bundled_image_wins_over_the_crop() {
    let only_the_swapped_body = |id: &str| id == SWAPPED_BODY;
    let shown = seat_pictures(
        &REPLICANT,
        &[Some(NORMAL_BODY), Some(SWAPPED_BODY), Some(SWAPPED_BODY)],
        only_the_swapped_body,
    );
    assert!(matches!(shown[0], SeatPicture::KeyArt(_)));
    let SeatPicture::Portrait { character_id, crop } = shown[1] else {
        panic!("the bundled image is shown: {:?}", shown[1]);
    };
    assert_eq!(character_id, SWAPPED_BODY);
    assert_eq!(crop.height_share, 1.0);
    assert!(!crop.mirrored);
    assert!(shown[2].crop().mirrored);
}

#[test]
fn a_seat_crop_keeps_the_area_shape_and_stays_inside_the_art() {
    let area = (256.0, 300.0);
    for body in [NORMAL_BODY, SWAPPED_BODY] {
        let seat = seat_pictures(&REPLICANT, &[Some(body)], no_portraits)[0].crop();
        let crop = crop_share_around_focus(HERO_SIZE_PX, area, seat.focus, seat.height_share);
        let shown_width = (crop.max_u - crop.min_u) * HERO_SIZE_PX.0;
        let shown_height = (crop.max_v - crop.min_v) * HERO_SIZE_PX.1;
        assert!((shown_width / shown_height - area.0 / area.1).abs() < 0.01);
        assert!(crop.min_u >= 0.0 && crop.max_u <= 1.0 && crop.min_v >= 0.0 && crop.max_v <= 1.0);
    }
}
