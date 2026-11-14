#[path = "channel.rs"]
mod channel;
#[path = "closing_session.rs"]
mod closing_session;
#[path = "config.rs"]
mod config;
#[path = "game_link.rs"]
mod game_link;
#[path = "home.rs"]
mod home;
#[path = "install.rs"]
mod install;
#[path = "lobby.rs"]
mod lobby;
#[path = "log_upload.rs"]
mod log_upload;
#[path = "logs.rs"]
mod logs;
#[path = "look.rs"]
mod look;
#[path = "proton.rs"]
mod proton;
#[path = "saves.rs"]
mod saves;
#[path = "scratch/mod.rs"]
mod scratch;
#[path = "seat_files.rs"]
mod seat_files;
#[path = "seat_round.rs"]
mod seat_round;
#[path = "seats.rs"]
mod seats;
#[path = "second_copy.rs"]
mod second_copy;
#[path = "steam_discovery.rs"]
mod steam_discovery;
#[path = "tabs.rs"]
mod tabs;
#[path = "update.rs"]
mod update;
#[path = "vdf.rs"]
mod vdf;

use std::collections::BTreeSet;
use std::path::Path;

#[test]
fn every_test_file_is_part_of_the_suite() {
    let tests_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let suite = include_str!("all.rs");
    let missing: BTreeSet<String> = std::fs::read_dir(&tests_dir)
        .expect("tests directory is readable")
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter_map(|name| name.strip_suffix(".rs").map(str::to_owned))
        .filter(|name| {
            name != "all" && name != "snapshots" && !suite.contains(&format!("mod {name};"))
        })
        .collect();
    assert!(
        missing.is_empty(),
        "test files missing from tests/all.rs: {missing:?}"
    );
}
