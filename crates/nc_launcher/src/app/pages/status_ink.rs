use eframe::egui::Color32;

use crate::app::palette::{NOT_READY, NOT_READY_ON_SAND, READY, READY_ON_SAND};
use crate::lobby::PlayerStatus;

/// Ready and in game read green-ish, every other status red-ish, on the ground or on a sand row.
pub(super) fn status_ink(status: PlayerStatus, on_sand: bool) -> Color32 {
    match (status.is_fine(), on_sand) {
        (true, false) => READY,
        (true, true) => READY_ON_SAND,
        (false, false) => NOT_READY,
        (false, true) => NOT_READY_ON_SAND,
    }
}
