use std::sync::atomic::{AtomicBool, Ordering};

use replicant_game::kaine::{
    companion_off_fights_from_scripts_in_use, scene_stand_ins_from_scripts_in_use,
};

use crate::diagnostics::log_line;

static ROWS_FROM_SCRIPTS: AtomicBool = AtomicBool::new(true);

/// Whether the companion rows generated from the script packs take part (`companion_rows_from_scripts`
/// in nier-coop.toml, on when unset); the hand-made rows always do.
pub(crate) fn companion_rows_from_scripts() -> bool {
    ROWS_FROM_SCRIPTS.load(Ordering::Relaxed)
}

pub(crate) fn set_companion_rows_from_scripts(enabled: bool) {
    ROWS_FROM_SCRIPTS.store(enabled, Ordering::Relaxed);
    log_line(&format!(
        "companion rows from the scripts: {} ({} fight rows and {} scene rows beside the hand-made ones; companion_rows_from_scripts in nier-coop.toml)",
        if enabled { "on" } else { "off" },
        companion_off_fights_from_scripts_in_use().count(),
        scene_stand_ins_from_scripts_in_use().count(),
    ));
}
