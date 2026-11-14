use eframe::egui::text::LayoutJob;
use eframe::egui::{Align, Color32, Id, Layout, Rect, TextFormat, Ui, UiBuilder, pos2, vec2};
use nc_launcher_core::{ART_FADE_MS, Easing, ROW_ART_BRIGHTEN_MS, banner_facts};

use crate::app::motion::{changed_since, ease_toward, with_clock};
use crate::app::pages::banner_shapes::paint_banner_shapes;
use crate::app::pages::game_title::game_title_job;
use crate::app::pages::lobby::session_open;
use crate::app::pages::tabs::lobby_tab;
use crate::app::pages::tone::tone_color;
use crate::app::palette::{INK, INK_MUTED, NOT_READY, READY};
use crate::app::parts::{
    LineTone, UPDATE_LINE_TOP_PADDING_PX, UpdateLineState, caps_galley, command_button,
    copy_line_inked, line_button, text_control, update_line, update_line_width,
};
use crate::app::style::{body_font, small_font};
use crate::app::{JoinDialog, Launcher, NetForms, Tab, UpdateState, game_look};
use crate::home::{BannerArtState, CardTone, CardView, GameCard, card_view};

use super::delete_saves_menu::open_on_right_click;
use super::row_art::{RowArtLight, paint_row_art};

const ART_LEFT_OFFSET_PX: f32 = 276.0;
const TEXT_COLUMN_WIDTH_PX: f32 = 420.0;
const TITLE_TOP_PX: f32 = 0.0;
const GARAMOND_TITLE_PX: f32 = 44.0;
const CAPS_TITLE_PX: f32 = 28.0;
const TITLE_LINE_HEIGHT_PX: f32 = 46.0;
const FACTS_TOP_PX: f32 = 78.0;
const FACT_KEY_WIDTH_PX: f32 = 80.0;
const FACT_ROW_HEIGHT_PX: f32 = 20.0;
const FACT_ROW_GAP_PX: f32 = 6.0;
const FOLDER_LINE_INSET_PX: f32 = 13.0;
const FOLDER_LINE_WIDTH_PX: f32 = 340.0;
const ACTIONS_BOTTOM_GAP_PX: f32 = 6.0;
const ACTION_HEIGHT_PX: f32 = 48.0;
const COMMAND_SIZE_PX: (f32, f32) = (176.0, 48.0);
const COMMAND_GAP_PX: f32 = 16.0;
const ACTION_GAP_PX: f32 = 22.0;
const QUIET_CONTROL_GAP_PX: f32 = 26.0;
const LINE_ACTION_SIDE_PADDING_PX: f32 = 20.0;
const FAILURE_LINE_GAP_PX: f32 = 3.0;
const EMPTY_FACT: &str = "\u{2014}";
const ELLIPSIS: char = '\u{2026}';
const ROW_ART_LOADED_SALT: &str = "row-art-loaded";
const ROW_ART_HOVER_SALT: &str = "row-art-hover";
const SWITCH_BLOCKED_NOTE: &str = "Finish the current game first.";
const SET_FOLDER_LABEL: &str = "Set the folder";
const HOST_LABEL: &str = "Host";
const JOIN_LABEL: &str = "Join";
const BACK_TO_LOBBY_LABEL: &str = "Lobby";
const SETTINGS_LABEL: &str = "Settings";
const UPDATING_TO_TEXT: &str = "updating to";
const UPDATING_TEXT: &str = "updating";
const MOD_TEXT_BEFORE_UPDATE: &str = ",";

pub(super) fn draw_game_row(
    ui: &mut Ui,
    rect: Rect,
    tab: &mut Tab,
    launcher: &mut Launcher,
    forms: &mut NetForms,
    card: &GameCard,
    switch_locked: bool,
) {
    let server_build = launcher.server_build_for(card.profile.game_id);
    let view = card_view(card, server_build.as_deref());
    let current = card.profile.game_id == launcher.profile.game_id;
    let found = !view.greyed_out;
    if found {
        open_on_right_click(ui, rect, launcher, forms, card.profile);
    }
    paint_art(ui, rect, launcher, card, found);
    let text = Rect::from_min_size(
        rect.min,
        vec2(TEXT_COLUMN_WIDTH_PX.min(rect.width()), rect.height()),
    );
    paint_title(ui, text, card, &view, found);
    let blocked = !current && switch_locked;
    let update = update_offer(launcher, card, &view, current, blocked);
    let (facts_bottom, update_clicked) = draw_facts(ui, text, &view, found, update.as_ref());
    if update_clicked {
        if view.update_from_server {
            launcher.start_update_apply(ui.ctx(), true);
        } else {
            launcher.install_mod_from_home(card.profile, ui.ctx());
        }
    }
    if found
        && let Some(failure) = forms
            .lobby_failure
            .as_ref()
            .filter(|failure| failure.game_id == card.profile.game_id)
    {
        let line = ui
            .painter()
            .layout_no_wrap(failure.reason.clone(), body_font(), NOT_READY);
        ui.painter().galley(
            pos2(text.left(), facts_bottom + FAILURE_LINE_GAP_PX),
            line,
            NOT_READY,
        );
    }
    let actions = Rect::from_min_size(
        pos2(
            text.left(),
            rect.bottom() - ACTIONS_BOTTOM_GAP_PX - ACTION_HEIGHT_PX,
        ),
        vec2(rect.width(), ACTION_HEIGHT_PX),
    );
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(actions)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing = vec2(ACTION_GAP_PX, 0.0);
            draw_actions(ui, tab, launcher, forms, card, &view, current, blocked);
        },
    );
}

fn paint_art(ui: &Ui, rect: Rect, launcher: &mut Launcher, card: &GameCard, found: bool) {
    let art_rect = Rect::from_min_max(
        pos2(rect.left() + ART_LEFT_OFFSET_PX, rect.top()),
        rect.right_bottom(),
    );
    if art_rect.width() <= 0.0 {
        return;
    }
    let painter = ui.painter_at(art_rect);
    let game_id = card.profile.game_id;
    let state = launcher.banner_art.state(ui.ctx(), game_id);
    let ready = matches!(state, BannerArtState::Ready(_));
    let loaded_since = changed_since(ui, Id::unique((ROW_ART_LOADED_SALT, game_id)), ready);
    let pointed = if ui.rect_contains_pointer(rect) {
        1.0
    } else {
        0.0
    };
    let light = RowArtLight {
        shown: with_clock(ui, |clock, _| {
            clock.entrance(loaded_since, 0.0, ART_FADE_MS, Easing::EaseOut)
        }),
        brightened: ease_toward(
            ui,
            Id::unique((ROW_ART_HOVER_SALT, game_id)),
            pointed,
            ROW_ART_BRIGHTEN_MS,
            Easing::EaseOut,
        ),
    };
    match state {
        BannerArtState::Ready(art) => {
            let texture = if found { &art.colour } else { &art.grey };
            paint_row_art(
                &painter,
                art_rect,
                texture,
                banner_facts(game_id).focus,
                light,
            );
        }
        BannerArtState::Missing => {
            paint_banner_shapes(&painter, art_rect, game_look(card.profile).mark, INK_MUTED);
        }
        BannerArtState::Pending => {}
    }
}

fn paint_title(ui: &Ui, text: Rect, card: &GameCard, view: &CardView, found: bool) {
    let painter = ui.painter_at(text);
    let color = if found { INK } else { INK_MUTED };
    let job = game_title_job(
        view.title,
        game_look(card.profile).title_face,
        GARAMOND_TITLE_PX,
        CAPS_TITLE_PX,
        color,
    );
    let title = painter.layout_job(job);
    let title_top = text.top() + TITLE_TOP_PX + (TITLE_LINE_HEIGHT_PX - title.size().y) / 2.0;
    painter.galley(pos2(text.left(), title_top), title, color);
}

fn update_offer(
    launcher: &Launcher,
    card: &GameCard,
    view: &CardView,
    current: bool,
    blocked: bool,
) -> Option<(String, UpdateLineState)> {
    let offered = view.update_text.clone()?;
    let updating = matches!(launcher.updater.state, UpdateState::Updating);
    if current && (launcher.busy() || updating) {
        let target = launcher
            .server_build_for(card.profile.game_id)
            .filter(|_| view.update_from_server)
            .or_else(|| card.ready_build.clone());
        let text = match target {
            Some(build) => format!("{UPDATING_TO_TEXT} {build}"),
            None => UPDATING_TEXT.to_owned(),
        };
        return Some((text, UpdateLineState::Running));
    }
    let checking = matches!(launcher.updater.state, UpdateState::Checking);
    let state = if blocked || launcher.busy() || updating || checking {
        UpdateLineState::Held
    } else {
        UpdateLineState::Offered
    };
    Some((offered, state))
}

fn draw_facts(
    ui: &mut Ui,
    text: Rect,
    view: &CardView,
    found: bool,
    update: Option<&(String, UpdateLineState)>,
) -> (f32, bool) {
    let mut top = text.top() + FACTS_TOP_PX;
    let value_left = text.left() + FACT_KEY_WIDTH_PX;
    let folder_ink = if view.found_tone == CardTone::Good {
        READY
    } else {
        NOT_READY
    };
    match (&view.folder_display, found) {
        (Some(folder), true) => {
            let line = Rect::from_min_size(
                pos2(value_left - FOLDER_LINE_INSET_PX, top),
                vec2(FOLDER_LINE_WIDTH_PX, text.height()),
            );
            let used = ui
                .scope_builder(UiBuilder::new().max_rect(line), |ui| {
                    copy_line_inked(
                        ui,
                        view.title,
                        folder,
                        &view.found_text,
                        FOLDER_LINE_WIDTH_PX,
                        small_font(),
                        folder_ink,
                    )
                    .on_hover_text(&view.found_text)
                    .rect
                    .height()
                })
                .inner;
            paint_key_centered(ui, "Folder", text.left(), top + used / 2.0, INK_MUTED);
            top += used + FACT_ROW_GAP_PX;
        }
        _ => {
            let shown = if view.found_text.is_empty() {
                EMPTY_FACT
            } else {
                &view.found_text
            };
            fact_row(ui, text, top, "Folder", shown, folder_ink, INK_MUTED);
            top += FACT_ROW_HEIGHT_PX + FACT_ROW_GAP_PX;
        }
    }
    if !found {
        fact_row(ui, text, top, "Mod", EMPTY_FACT, INK_MUTED, INK_MUTED);
        return (top + FACT_ROW_HEIGHT_PX, false);
    }
    let Some((line_text, state)) = update else {
        let mod_ink = tone_color(view.mod_tone);
        // The same place the line takes when an update follows it, so the row does not move.
        let text_height = ui
            .painter()
            .layout_no_wrap(view.mod_text.clone(), body_font(), mod_ink)
            .size()
            .y;
        let row_top = top + UPDATE_LINE_TOP_PADDING_PX + (text_height - FACT_ROW_HEIGHT_PX) / 2.0;
        fact_row(ui, text, row_top, "Mod", &view.mod_text, mod_ink, INK_MUTED);
        return (row_top + FACT_ROW_HEIGHT_PX, false);
    };
    draw_mod_with_update(ui, text, top, view, line_text, *state)
}

fn draw_mod_with_update(
    ui: &mut Ui,
    text: Rect,
    top: f32,
    view: &CardView,
    line_text: &str,
    state: UpdateLineState,
) -> (f32, bool) {
    let value_left = text.left() + FACT_KEY_WIDTH_PX;
    let mod_ink = tone_color(view.mod_tone);
    let mod_galley = ui.painter().layout_no_wrap(
        format!("{}{MOD_TEXT_BEFORE_UPDATE}", view.mod_text),
        body_font(),
        mod_ink,
    );
    let text_top = top + UPDATE_LINE_TOP_PADDING_PX;
    let mod_width = mod_galley.size().x;
    ui.painter()
        .galley(pos2(value_left, text_top), mod_galley, mod_ink);
    // The control starts at the comma; its own padding is the gap before the update's text.
    let control_left = value_left + mod_width;
    let width = update_line_width(ui, line_text);
    let line = Rect::from_min_size(pos2(control_left, top), vec2(width, text.height()));
    let (clicked, height) = ui
        .scope_builder(UiBuilder::new().max_rect(line), |ui| {
            let response = update_line(ui, view.title, line_text, width, state);
            (response.clicked(), response.rect.height())
        })
        .inner;
    paint_key_centered(ui, "Mod", text.left(), top + height / 2.0, INK_MUTED);
    (top + height, clicked)
}

fn fact_row(
    ui: &Ui,
    text: Rect,
    top: f32,
    key: &str,
    value: &str,
    value_color: Color32,
    key_color: Color32,
) {
    let middle = top + FACT_ROW_HEIGHT_PX / 2.0;
    let key_galley = caps_galley(ui.painter(), key, key_color);
    let key_top = middle - key_galley.size().y / 2.0;
    ui.painter()
        .galley(pos2(text.left(), key_top), key_galley, key_color);
    let value_left = text.left() + FACT_KEY_WIDTH_PX;
    let painter = ui.painter_at(Rect::from_min_max(
        pos2(value_left, top),
        pos2(text.right(), top + FACT_ROW_HEIGHT_PX),
    ));
    let mut job = LayoutJob::single_section(
        value.to_owned(),
        TextFormat {
            font_id: body_font(),
            color: value_color,
            ..TextFormat::default()
        },
    );
    job.wrap.max_width = text.right() - value_left;
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some(ELLIPSIS);
    let galley = painter.layout_job(job);
    painter.galley(
        pos2(value_left, middle - galley.size().y / 2.0),
        galley,
        value_color,
    );
}

fn paint_key_centered(ui: &Ui, key: &str, left: f32, middle: f32, color: Color32) {
    let galley = caps_galley(ui.painter(), key, color);
    let top = middle - galley.size().y / 2.0;
    ui.painter().galley(pos2(left, top), galley, color);
}

fn draw_actions(
    ui: &mut Ui,
    tab: &mut Tab,
    launcher: &mut Launcher,
    forms: &mut NetForms,
    card: &GameCard,
    view: &CardView,
    current: bool,
    blocked: bool,
) {
    if !view.play_enabled {
        let set_folder = line_button(
            ui,
            SET_FOLDER_LABEL,
            LineTone::Plain,
            ACTION_HEIGHT_PX,
            Some(LINE_ACTION_SIDE_PADDING_PX),
            !blocked,
        );
        if set_folder.clicked() {
            launcher.open_game(card.profile);
            *tab = Tab::Settings;
        }
    } else if current && session_open(forms) {
        if command_button(ui, BACK_TO_LOBBY_LABEL, COMMAND_SIZE_PX, true, true).clicked() {
            launcher.open_game(card.profile);
            *tab = lobby_tab(launcher.lobby_mode);
        }
    } else {
        draw_entry_commands(ui, launcher, forms, card, blocked);
    }
    if view.play_enabled
        && let Some(label) = view.mod_action
    {
        let enabled = !blocked && !launcher.busy();
        let action = line_button(
            ui,
            label,
            LineTone::Warm,
            ACTION_HEIGHT_PX,
            Some(LINE_ACTION_SIDE_PADDING_PX),
            enabled,
        );
        if action.clicked() {
            launcher.install_mod_from_home(card.profile, ui.ctx());
        }
    }
    ui.add_space(QUIET_CONTROL_GAP_PX - ACTION_GAP_PX);
    if text_control(ui, SETTINGS_LABEL, !blocked).clicked() {
        launcher.open_game(card.profile);
        *tab = Tab::Settings;
    }
}

fn draw_entry_commands(
    ui: &mut Ui,
    launcher: &mut Launcher,
    forms: &mut NetForms,
    card: &GameCard,
    blocked: bool,
) {
    let available =
        !blocked && !session_open(forms) && !forms.hosting_pending() && !forms.joining_pending();
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = COMMAND_GAP_PX;
        let mut host = command_button(ui, HOST_LABEL, COMMAND_SIZE_PX, true, available);
        let mut join = command_button(ui, JOIN_LABEL, COMMAND_SIZE_PX, false, available);
        if blocked {
            host = host.on_hover_text(SWITCH_BLOCKED_NOTE);
            join = join.on_hover_text(SWITCH_BLOCKED_NOTE);
        }
        if host.clicked() && launcher.choose_game(card.profile) {
            launcher.take_hosting_kind();
            let now_ms = ui.input(|input| input.time) * 1000.0;
            forms.start_host(ui.ctx(), now_ms);
        }
        if join.clicked() && launcher.choose_game(card.profile) {
            forms.join_dialog = Some(JoinDialog::default());
        }
    });
}
