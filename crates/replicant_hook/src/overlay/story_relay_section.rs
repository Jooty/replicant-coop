use hudhook::imgui::Ui;
use nc_hook::menu_view::parts::switch_line;

use crate::overlay::boss_messages_section::draw_boss_messages;
use crate::story_relay::{set_shared_story_events, shared_story_events};

pub(super) fn draw_story_relay(ui: &Ui) {
    ui.separator();
    let mut shared = shared_story_events();
    if switch_line(ui, "Shared story events", &mut shared) {
        set_shared_story_events(shared);
    }
    draw_boss_messages(ui);
}
