#[path = "banner_art.rs"]
mod banner_art;
#[path = "boot_console.rs"]
mod boot_console;
#[path = "bridge.rs"]
mod bridge;
#[path = "campaign_flow.rs"]
mod campaign_flow;
#[path = "coop_save_delete.rs"]
mod coop_save_delete;
#[path = "copy_feedback.rs"]
mod copy_feedback;
#[path = "followed_kind.rs"]
mod followed_kind;
#[path = "home_entry.rs"]
mod home_entry;
#[path = "iroh.rs"]
mod iroh;
#[path = "launcher_scale.rs"]
mod launcher_scale;
#[path = "lobby_rules.rs"]
mod lobby_rules;
#[path = "log_upload.rs"]
mod log_upload;
#[path = "members.rs"]
mod members;
#[path = "motion.rs"]
mod motion;
#[path = "options.rs"]
mod options;
#[path = "profiles.rs"]
mod profiles;
#[path = "replicant_session.rs"]
mod replicant_session;
#[path = "report.rs"]
mod report;
#[path = "seat_assembler.rs"]
mod seat_assembler;
#[path = "session_end.rs"]
mod session_end;

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
        .filter(|name| name != "all" && !suite.contains(&format!("mod {name};")))
        .collect();
    assert!(
        missing.is_empty(),
        "test files missing from tests/all.rs: {missing:?}"
    );
}
