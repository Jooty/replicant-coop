use eframe::egui::{Rect, pos2};

pub(super) const LOBBY_BAR_HEIGHT_PX: f32 = 84.0;
const MENU_COLUMN_WIDTH_PX: f32 = 288.0;
const MENU_TO_MAIN_GAP_PX: f32 = 24.0;
const COLUMNS_TOP_NUDGE_PX: f32 = 4.0;
const COLUMNS_TO_BAR_GAP_PX: f32 = 46.0;

/// Where the lobby's parts go: the menu column at the left, the players at the right, and the command bar
/// across the whole window's foot.
pub(super) struct LobbyLayout {
    pub menu: Rect,
    pub main: Rect,
    pub bar: Rect,
}

pub(super) fn lobby_layout(page: Rect, window: Rect) -> LobbyLayout {
    let bar = Rect::from_min_max(
        pos2(window.left(), window.bottom() - LOBBY_BAR_HEIGHT_PX),
        window.right_bottom(),
    );
    let top = page.top() + COLUMNS_TOP_NUDGE_PX;
    let bottom = (bar.top() - COLUMNS_TO_BAR_GAP_PX).max(top);
    let menu_right = page.left() + MENU_COLUMN_WIDTH_PX;
    LobbyLayout {
        menu: Rect::from_min_max(pos2(page.left(), top), pos2(menu_right, bottom)),
        main: Rect::from_min_max(
            pos2(menu_right + MENU_TO_MAIN_GAP_PX, top),
            pos2(page.right(), bottom),
        ),
        bar,
    }
}
