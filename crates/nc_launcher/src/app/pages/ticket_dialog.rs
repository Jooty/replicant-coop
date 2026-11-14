use eframe::egui::{
    Align, Area, Color32, Id, Key, Layout, Order, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder,
    pos2, vec2,
};
use nc_launcher_core::ticket_check;

use crate::app::pages::game_title::game_title_job;
use crate::app::pages::ticket_field::ticket_field;
use crate::app::palette::{BAND, HAIRLINE_STRONG, HARD_SHADOW, INK, INK_MUTED, NOT_READY};
use crate::app::parts::{caps_galley, command_button};
use crate::app::style::small_font;
use crate::app::{Launcher, NetForms, game_look};
use crate::net::LOBBY_MODE_NEW_GAME;

const DIALOG_SIZE_PX: (f32, f32) = (520.0, 248.0);
const DIALOG_PADDING_PX: f32 = 28.0;
const DIALOG_SHADOW_OFFSET_PX: f32 = 6.0;
const DIM_ALPHA: u8 = 170;
const TITLE_GARAMOND_PX: f32 = 30.0;
const TITLE_CAPS_PX: f32 = 22.0;
const TITLE_TO_TICKET_PX: f32 = 26.0;
const TICKET_KEY_WIDTH_PX: f32 = 70.0;
const TICKET_LINE_HEIGHT_PX: f32 = 33.0;
const TICKET_TO_REFUSAL_PX: f32 = 8.0;
const REFUSAL_LINE_HEIGHT_PX: f32 = 18.0;
const COMMAND_SIZE_PX: (f32, f32) = (176.0, 48.0);
const DIM_LAYER: &str = "join-dialog-dim";
const DIALOG_LAYER: &str = "join-dialog";
const TICKET_KEY: &str = "Ticket";
const JOIN_LABEL: &str = "Join";

pub(super) fn draw_ticket_dialog(ui: &mut Ui, launcher: &mut Launcher, forms: &mut NetForms) {
    let whole = ui.ctx().content_rect();
    let mut close = ui.input(|input| input.key_pressed(Key::Escape));
    Area::new(Id::unique(DIM_LAYER))
        .order(Order::Middle)
        .fixed_pos(whole.min)
        .show(ui.ctx(), |ui| {
            let (rect, response) = ui.allocate_exact_size(whole.size(), Sense::click());
            ui.painter()
                .rect_filled(rect, 0.0, Color32::from_black_alpha(DIM_ALPHA));
            close |= response.clicked();
        });
    let dialog = Rect::from_center_size(whole.center(), vec2(DIALOG_SIZE_PX.0, DIALOG_SIZE_PX.1));
    Area::new(Id::unique(DIALOG_LAYER))
        .order(Order::Foreground)
        .fixed_pos(dialog.min)
        .show(ui.ctx(), |ui| {
            ui.set_min_size(dialog.size());
            draw_dialog(ui, dialog, launcher, forms);
        });
    if close {
        forms.join_dialog = None;
    }
}

fn draw_dialog(ui: &mut Ui, dialog: Rect, launcher: &mut Launcher, forms: &mut NetForms) {
    let painter = ui.painter().clone();
    painter.rect_filled(
        dialog.translate(vec2(DIALOG_SHADOW_OFFSET_PX, DIALOG_SHADOW_OFFSET_PX)),
        0.0,
        HARD_SHADOW,
    );
    painter.rect_filled(dialog, 0.0, BAND);
    painter.rect_stroke(
        dialog,
        0.0,
        Stroke::new(1.0, HAIRLINE_STRONG),
        StrokeKind::Inside,
    );
    let inner = dialog.shrink(DIALOG_PADDING_PX);
    let title = painter.layout_job(game_title_job(
        launcher.profile.display_name,
        game_look(launcher.profile).title_face,
        TITLE_GARAMOND_PX,
        TITLE_CAPS_PX,
        INK,
    ));
    let title_height = title.size().y;
    painter.galley(inner.min, title, INK);
    let line_top = inner.top() + title_height + TITLE_TO_TICKET_PX;
    let key = caps_galley(&painter, TICKET_KEY, INK_MUTED);
    let key_top = line_top + (TICKET_LINE_HEIGHT_PX - key.size().y) / 2.0;
    painter.galley(pos2(inner.left(), key_top), key, INK_MUTED);
    let line = Rect::from_min_max(
        pos2(inner.left() + TICKET_KEY_WIDTH_PX, line_top),
        pos2(inner.right(), line_top + TICKET_LINE_HEIGHT_PX),
    );
    let before = forms.join_ticket.clone();
    let field = ticket_field(ui, line, &mut forms.join_ticket);
    let changed = forms.join_ticket != before;
    let Some(state) = forms.join_dialog.as_mut() else {
        return;
    };
    if !state.focused_once {
        field.request_focus();
        state.focused_once = true;
    }
    if changed {
        state.refusal = None;
    }
    if let Some(refusal) = state.refusal {
        let refusal_top = line.bottom() + TICKET_TO_REFUSAL_PX;
        let galley = painter.layout_no_wrap(refusal.to_owned(), small_font(), NOT_READY);
        let top = refusal_top + (REFUSAL_LINE_HEIGHT_PX - galley.size().y) / 2.0;
        painter.galley(pos2(line.left(), top), galley, NOT_READY);
    }
    let entered = field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
    let has_ticket = !forms.join_ticket.trim().is_empty();
    let command_row = Rect::from_min_max(
        pos2(inner.left(), inner.bottom() - COMMAND_SIZE_PX.1),
        inner.right_bottom(),
    );
    let pressed = ui
        .scope_builder(
            UiBuilder::new()
                .max_rect(command_row)
                .layout(Layout::right_to_left(Align::Center)),
            |ui| command_button(ui, JOIN_LABEL, COMMAND_SIZE_PX, true, has_ticket).clicked(),
        )
        .inner;
    if !(pressed || entered) || !has_ticket {
        return;
    }
    match ticket_check(&forms.join_ticket) {
        Err(refusal) => {
            if let Some(state) = forms.join_dialog.as_mut() {
                state.refusal = Some(refusal);
            }
            field.request_focus();
        }
        Ok(()) => {
            let now_ms = ui.input(|input| input.time) * 1000.0;
            launcher.lobby_mode = LOBBY_MODE_NEW_GAME;
            forms.start_join(ui.ctx(), now_ms);
        }
    }
    ui.ctx().request_repaint();
}
