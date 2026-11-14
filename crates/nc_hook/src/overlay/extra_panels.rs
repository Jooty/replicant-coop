use hudhook::imgui::Ui;

use crate::menu_view::MenuState;
use crate::overlay::OverlayGame;

pub(super) fn draw_extra_panels<G: OverlayGame>(ui: &Ui, game: &mut G, state: &mut MenuState) {
    for panel in game.extra_panels() {
        let mut open = state.panel_is_open(panel.id);
        if open {
            game.draw_extra_panel(ui, panel.id, &mut open);
            state.set_panel_open(panel.id, open);
        }
    }
}
