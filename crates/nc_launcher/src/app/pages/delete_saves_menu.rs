use eframe::egui::{
    Area, Color32, Id, Key, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};
use nc_launcher_core::GameProfile;
use nc_ui_style::{LINE_BUTTON_HEIGHT_PX, LINE_BUTTON_SIDE_PADDING_PX};

use crate::app::pages::lobby::session_open;
use crate::app::palette::{
    BAND, HAIRLINE_STRONG, HARD_SHADOW, HATCH_LINE, HOVER_WASH, INK, INK_UNAVAILABLE, NOT_READY,
};
use crate::app::parts::{caps_galley, paint_hatching};
use crate::app::{Launcher, NetForms};

const MENU_LAYER: &str = "coop-save-menu";
const MENU_ENTRY_SALT: &str = "coop-save-menu-entry";
const MENU_PADDING_PX: f32 = 6.0;
const MENU_SHADOW_OFFSET_PX: f32 = 4.0;
const MENU_EDGE_GAP_PX: f32 = 8.0;
const DELETE_LABEL: &str = "Delete co-op saves";
const CONFIRM_LABEL: &str = "Confirm delete";

pub(super) fn open_on_right_click(
    ui: &Ui,
    row: Rect,
    launcher: &mut Launcher,
    forms: &NetForms,
    profile: &'static GameProfile,
) {
    if forms.join_dialog.is_some() || !profile.support.saves {
        return;
    }
    // Anywhere on the row counts, over its buttons too.
    let shown = row.intersect(ui.clip_rect());
    let clicked_at = ui.input(|input| {
        input
            .pointer
            .secondary_clicked()
            .then(|| input.pointer.interact_pos())
            .flatten()
    });
    if let Some(at) = clicked_at.filter(|at| shown.contains(*at)) {
        launcher.open_coop_save_menu(profile, at);
    }
}

pub(super) fn draw_delete_saves_menu(ui: &mut Ui, launcher: &mut Launcher, forms: &NetForms) {
    let Some(menu) = launcher.coop_save_menu.clone() else {
        return;
    };
    let lobby_open = lobby_open_for(launcher, forms, menu.profile);
    let Some(offer) = launcher.coop_save_menu_offer(lobby_open) else {
        return;
    };
    let painter = ui.painter();
    let label_width = [DELETE_LABEL, CONFIRM_LABEL]
        .iter()
        .map(|label| caps_galley(painter, label, INK).size().x)
        .fold(0.0, f32::max);
    let size = vec2(
        label_width + 2.0 * (LINE_BUTTON_SIDE_PADDING_PX + MENU_PADDING_PX),
        LINE_BUTTON_HEIGHT_PX + 2.0 * MENU_PADDING_PX,
    );
    let whole = ui.ctx().content_rect().shrink(MENU_EDGE_GAP_PX);
    let menu_rect = Rect::from_min_size(placed_inside(menu.anchor, size, whole), size);
    let (pressed_outside, escaped) = ui.input(|input| {
        let outside = input.pointer.any_pressed()
            && input
                .pointer
                .press_origin()
                .is_some_and(|at| !menu_rect.contains(at));
        (outside, input.key_pressed(Key::Escape))
    });
    let mut chosen = false;
    Area::new(Id::unique(MENU_LAYER))
        .order(Order::Foreground)
        .fixed_pos(menu_rect.min)
        .show(ui.ctx(), |ui| {
            ui.allocate_exact_size(size, Sense::hover());
            let painter = ui.painter();
            painter.rect_filled(
                menu_rect.translate(vec2(MENU_SHADOW_OFFSET_PX, MENU_SHADOW_OFFSET_PX)),
                0.0,
                HARD_SHADOW,
            );
            painter.rect_filled(menu_rect, 0.0, BAND);
            painter.rect_stroke(
                menu_rect,
                0.0,
                Stroke::new(1.0, HAIRLINE_STRONG),
                StrokeKind::Inside,
            );
            let entry = menu_rect.shrink(MENU_PADDING_PX);
            chosen = draw_entry(ui, entry, menu.confirming, offer.allowed());
        });
    if escaped || pressed_outside {
        launcher.coop_save_menu = None;
    } else if chosen && menu.confirming {
        launcher.delete_coop_saves_of(menu.profile, lobby_open);
    } else if chosen && let Some(open) = launcher.coop_save_menu.as_mut() {
        open.confirming = true;
    }
}

fn draw_entry(ui: &mut Ui, entry: Rect, confirming: bool, allowed: bool) -> bool {
    let sense = if allowed {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.interact(entry, Id::unique(MENU_ENTRY_SALT), sense);
    let painter = ui.painter();
    let (label, ink, border): (&str, Color32, Option<Color32>) = match (allowed, confirming) {
        (false, _) => (DELETE_LABEL, INK_UNAVAILABLE, None),
        (true, false) => (DELETE_LABEL, INK, None),
        (true, true) => (CONFIRM_LABEL, NOT_READY, Some(NOT_READY)),
    };
    if !allowed {
        paint_hatching(painter, entry, HATCH_LINE);
    } else if response.hovered() {
        painter.rect_filled(entry, 0.0, HOVER_WASH);
    }
    if let Some(border) = border {
        painter.rect_stroke(entry, 0.0, Stroke::new(1.0, border), StrokeKind::Inside);
    }
    let galley = caps_galley(painter, label, ink);
    let at = pos2(
        entry.left() + LINE_BUTTON_SIDE_PADDING_PX,
        entry.center().y - galley.size().y / 2.0,
    );
    painter.galley(at, galley, ink);
    response.clicked()
}

fn placed_inside(anchor: Pos2, size: eframe::egui::Vec2, whole: Rect) -> Pos2 {
    pos2(
        anchor.x.min(whole.right() - size.x).max(whole.left()),
        anchor.y.min(whole.bottom() - size.y).max(whole.top()),
    )
}

fn lobby_open_for(launcher: &Launcher, forms: &NetForms, profile: &GameProfile) -> bool {
    profile.game_id == launcher.profile.game_id
        && (launcher.lobby_started
            || session_open(forms)
            || forms.hosting_pending()
            || forms.joining_pending())
}
