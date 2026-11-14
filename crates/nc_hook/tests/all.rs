#[path = "action_queue.rs"]
mod action_queue;
#[path = "area_change.rs"]
mod area_change;
#[path = "crash_report.rs"]
mod crash_report;
#[path = "hud_guard.rs"]
mod hud_guard;
#[path = "log_stamp.rs"]
mod log_stamp;
#[path = "menu_model.rs"]
mod menu_model;
#[path = "net_worker.rs"]
mod net_worker;
#[path = "page_protection.rs"]
mod page_protection;
#[path = "pe_identity.rs"]
mod pe_identity;
#[path = "relay_inbox.rs"]
mod relay_inbox;
#[path = "stall_watch.rs"]
mod stall_watch;

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
            name != "all" && name != "menu_snapshots" && !suite.contains(&format!("mod {name};"))
        })
        .collect();
    assert!(
        missing.is_empty(),
        "test files missing from tests/all.rs: {missing:?}"
    );
}
