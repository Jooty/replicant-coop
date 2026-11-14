use eframe::egui::Color32;

use crate::app::palette::{INK, INK_MUTED, NOT_READY};
use crate::home::CardTone;

pub(super) fn tone_color(tone: CardTone) -> Color32 {
    match tone {
        CardTone::Good => INK,
        CardTone::Warn | CardTone::Bad => NOT_READY,
        CardTone::Muted => INK_MUTED,
    }
}
