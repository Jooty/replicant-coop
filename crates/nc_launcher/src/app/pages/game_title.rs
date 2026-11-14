use eframe::egui::text::LayoutJob;
use eframe::egui::{Color32, FontId};

use crate::app::TitleFace;
use crate::app::parts::spaced_job;
use crate::app::style::{garamond_family, light_family};

const SPACED_TITLE_LETTER_SPACING_EM: f32 = 0.26;

pub(super) fn game_title_job(
    name: &str,
    face: TitleFace,
    garamond_size: f32,
    caps_size: f32,
    color: Color32,
) -> LayoutJob {
    match face {
        TitleFace::Garamond => spaced_job(
            name,
            FontId::new(garamond_size, garamond_family()),
            color,
            0.0,
        ),
        TitleFace::SpacedCapitals => spaced_job(
            &name.to_uppercase(),
            FontId::new(caps_size, light_family()),
            color,
            SPACED_TITLE_LETTER_SPACING_EM,
        ),
    }
}
