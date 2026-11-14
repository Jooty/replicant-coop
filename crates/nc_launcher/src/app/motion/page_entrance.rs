use std::hash::Hash;

use eframe::egui::{Id, Ui};
use nc_launcher_core::{SECTION_ENTER_MS, SECTION_STAGGER_MS};

use crate::app::motion::{changed_since, entering};

const PAGE_SALT: &str = "launcher-page";
const PAGE_SINCE_SALT: &str = "launcher-page-since";

/// Notes which page is drawn this frame; a new page's sections come in from now on.
pub fn mark_page(ui: &Ui, page: impl Hash) {
    let since = changed_since(ui, Id::unique(PAGE_SALT), page);
    ui.ctx()
        .data_mut(|data| data.insert_temp(Id::unique(PAGE_SINCE_SALT), since));
}

pub fn page_since(ui: &Ui) -> f64 {
    ui.ctx()
        .data(|data| data.get_temp::<f64>(Id::unique(PAGE_SINCE_SALT)))
        .unwrap_or(f64::NEG_INFINITY)
}

/// One section of the page, coming in after the `index` sections before it.
pub fn page_section<R>(ui: &mut Ui, index: usize, add: impl FnOnce(&mut Ui) -> R) -> R {
    let delay = index as f32 * SECTION_STAGGER_MS;
    entering(ui, page_since(ui), delay, SECTION_ENTER_MS, add)
}
