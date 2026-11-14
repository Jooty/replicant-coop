use std::hash::{DefaultHasher, Hash, Hasher};

use eframe::egui::{Id, Ui};

/// When the thing known by `id` last changed to `key`, on the launcher's clock. A thing first seen has always
/// been as it is, so nothing animates it in.
pub fn changed_since(ui: &Ui, id: Id, key: impl Hash) -> f64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    let key = hasher.finish();
    let now = ui.input(|input| input.time);
    ui.ctx().data_mut(|data| {
        let seen = data.get_temp_mut_or_insert_with(id, || (key, f64::NEG_INFINITY));
        if seen.0 != key {
            *seen = (key, now);
        }
        seen.1
    })
}
