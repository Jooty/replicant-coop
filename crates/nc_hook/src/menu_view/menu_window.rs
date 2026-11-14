use imgui::{Condition, ItemHoveredFlags, Ui, WindowFlags};
use nc_ui_style::{INK, INK_MUTED, NOT_READY, READY};

use crate::build_gate::GateError;
use crate::diagnostics::{log_line, log_path};
use crate::menu::{
    AreaEntry, CoOpStatus, ExtraPanel, FixEntry, FixOutcome, GameMenu, MenuNote, MenuSection,
    NoteTone, SectionBody, StoryJumpEntry, build_menu,
};
use crate::menu_view::MenuState;
use crate::menu_view::menu_sizes::{
    MENU_AREA_LIST_HEIGHT_PX, MENU_HEADING_TO_BODY_GAP_PX, MENU_SECTION_GAP_PX,
    MENU_WINDOW_POSITION_PX, MENU_WINDOW_SIZE_PX,
};
use crate::menu_view::parts::{
    fact_line, field_line, fold_heading, line_buttons, menu_row, note_line, paint_window_shadow,
    sub_heading,
};

const MAP_TELEPORTER_HEADING: &str = "map teleporter";
const LOG_FILE_KEY: &str = "log file";
const BUILD_GATE_KEY: &str = "build gate";

pub fn watch_area_changes(game: &dyn GameMenu, state: &mut MenuState) {
    if !state.area_changes.is_waiting() {
        return;
    }
    let now = state.now_ms();
    if let Some(outcome) = state
        .area_changes
        .observe(game.current_area().as_ref(), now)
    {
        log_line(&outcome.log_line());
    }
}

/// The mod's menu: one folding section per part of the model, in the launcher's look. `draw_settings` draws
/// the game's own settings at the top of the settings section.
pub fn draw_menu_window<G: GameMenu>(
    ui: &Ui,
    game: &mut G,
    mut draw_settings: impl FnMut(&mut G, &Ui),
    gate_error: Option<&GateError>,
    state: &mut MenuState,
) {
    let model = build_menu(&*game);
    ui.window("menu##debug")
        .position(MENU_WINDOW_POSITION_PX, Condition::FirstUseEver)
        .size(MENU_WINDOW_SIZE_PX, Condition::FirstUseEver)
        .flags(WindowFlags::NO_TITLE_BAR | WindowFlags::NO_COLLAPSE)
        .build(|| {
            paint_window_shadow(ui);
            for (index, section) in model.sections.iter().enumerate() {
                if index > 0 {
                    ui.dummy([0.0, MENU_SECTION_GAP_PX]);
                }
                let _id = ui.push_id(section.kind.title());
                let mut open = state.section_is_open(section.kind);
                if fold_heading(ui, section.kind.title(), &mut open) {
                    state.set_section_open(section.kind, open);
                }
                if !open {
                    continue;
                }
                ui.dummy([0.0, MENU_HEADING_TO_BODY_GAP_PX]);
                draw_section(ui, game, &mut draw_settings, gate_error, state, section);
                draw_notes(ui, &section.notes);
                if let Some(note) = &section.empty_note {
                    note_line(ui, note, INK_MUTED);
                }
            }
        });
}

fn draw_section<G: GameMenu>(
    ui: &Ui,
    game: &mut G,
    draw_settings: &mut impl FnMut(&mut G, &Ui),
    gate_error: Option<&GateError>,
    state: &mut MenuState,
    section: &MenuSection,
) {
    match &section.body {
        SectionBody::CoOp(status) => draw_co_op(ui, status),
        SectionBody::Teleport { current, areas } => {
            draw_teleport(ui, game, state, current.as_ref(), areas);
        }
        SectionBody::StoryJump(jumps) => draw_story_jumps(ui, game, state, jumps),
        SectionBody::Fixes(fixes) => draw_fixes(ui, game, state, fixes),
        SectionBody::SettingsAndLogs {
            status_lines,
            panels,
        } => {
            draw_settings(game, ui);
            ui.dummy([0.0, MENU_SECTION_GAP_PX]);
            if let Some(error) = gate_error {
                fact_line(ui, BUILD_GATE_KEY, &format!("FAIL ({error})"), NOT_READY);
            }
            for line in status_lines {
                draw_status_line(ui, line);
            }
            fact_line(ui, LOG_FILE_KEY, log_path(), INK_MUTED);
            ui.dummy([0.0, MENU_HEADING_TO_BODY_GAP_PX]);
            draw_panel_buttons(ui, state, panels);
        }
    }
}

fn draw_co_op(ui: &Ui, status: &CoOpStatus) {
    let link_ink = if status.connected { READY } else { NOT_READY };
    fact_line(ui, "link", &status.link, link_ink);
    for player in &status.players {
        fact_line(ui, &player.name, &player.detail, INK);
    }
    match status.ping_ms {
        Some(ping) => fact_line(ui, "ping", &format!("{ping} ms"), INK),
        None => fact_line(ui, "ping", "unknown", INK_MUTED),
    }
}

fn draw_status_line(ui: &Ui, line: &str) {
    match line.split_once(": ") {
        Some((key, value)) => fact_line(ui, key, value, INK),
        None => note_line(ui, line, INK_MUTED),
    }
}

fn draw_teleport<G: GameMenu>(
    ui: &Ui,
    game: &G,
    state: &mut MenuState,
    current: Option<&AreaEntry>,
    areas: &[AreaEntry],
) {
    match line_buttons(ui, &["Go to partner", "Bring partner here"], &[]) {
        Some(0) => state.teleport_message = describe(game.go_to_partner()),
        Some(_) => state.teleport_message = describe(game.bring_partner()),
        None => {}
    }
    if !state.teleport_message.is_empty() {
        note_line(ui, &state.teleport_message, INK_MUTED);
    }
    ui.dummy([0.0, MENU_HEADING_TO_BODY_GAP_PX]);
    sub_heading(ui, MAP_TELEPORTER_HEADING);
    match current {
        Some(area) => fact_line(ui, "current", &area.name, INK),
        None => fact_line(ui, "current", "unknown", INK_MUTED),
    }
    if areas.is_empty() {
        return;
    }
    field_line(ui, "filter", &mut state.area_filter);
    let filter = state.area_filter.to_ascii_lowercase();
    let mut chosen: Option<&AreaEntry> = None;
    ui.child_window("area list")
        .size([0.0, MENU_AREA_LIST_HEIGHT_PX])
        .border(false)
        .build(|| {
            for area in areas
                .iter()
                .filter(|area| area.name.to_ascii_lowercase().contains(&filter))
            {
                let here = current.is_some_and(|current| current.id == area.id);
                let _id = ui.push_id_usize(area.id as usize);
                if menu_row(ui, &area.name, None, here, true) {
                    chosen = Some(area);
                }
            }
        });
    if let Some(target) = chosen {
        request_area_change(game, state, current, target);
    }
    let message = state.area_changes.message();
    if !message.is_empty() {
        note_line(ui, &message, INK_MUTED);
    }
}

fn request_area_change<G: GameMenu>(
    game: &G,
    state: &mut MenuState,
    current: Option<&AreaEntry>,
    target: &AreaEntry,
) {
    log_line(&format!(
        "teleport: asked to change the area to {}",
        target.name
    ));
    let now = state.now_ms();
    match state
        .area_changes
        .request(target, current, game.area_change_blocker(), now)
    {
        Err(refusal) => log_line(&format!("teleport: refused ({refusal})")),
        Ok(()) => {
            if let Err(reason) = game.change_area(target) {
                state.area_changes.game_refused(&reason);
                log_line(&format!("teleport: refused ({reason})"));
            }
        }
    }
}

fn draw_story_jumps<G: GameMenu>(
    ui: &Ui,
    game: &G,
    state: &mut MenuState,
    jumps: &[StoryJumpEntry],
) {
    for jump in jumps {
        let _id = ui.push_id_usize(jump.id as usize);
        if menu_row(ui, &jump.label, None, false, true) {
            state.story_jump_message = describe(game.run_story_jump(jump.id));
        }
    }
    if !state.story_jump_message.is_empty() {
        note_line(ui, &state.story_jump_message, INK_MUTED);
    }
}

fn draw_fixes<G: GameMenu>(ui: &Ui, game: &G, state: &mut MenuState, fixes: &[FixEntry]) {
    for fix in fixes {
        let _id = ui.push_id_usize(fix.id as usize);
        let pressed = menu_row(ui, &fix.label, None, false, fix.disabled_reason.is_none());
        if let Some(reason) = &fix.disabled_reason
            && ui.is_item_hovered_with_flags(ItemHoveredFlags::ALLOW_WHEN_DISABLED)
        {
            ui.tooltip_text(reason);
        }
        if pressed {
            state.fix_message = match game.run_fix(fix.id) {
                FixOutcome::Done => String::new(),
                FixOutcome::CopyToClipboard(text) => {
                    ui.set_clipboard_text(text);
                    String::from("copied to the clipboard")
                }
                FixOutcome::Refused(reason) => reason,
            };
        }
    }
    if !state.fix_message.is_empty() {
        note_line(ui, &state.fix_message, INK_MUTED);
    }
}

fn draw_panel_buttons(ui: &Ui, state: &mut MenuState, panels: &[ExtraPanel]) {
    let labels: Vec<&str> = panels.iter().map(|panel| panel.label).collect();
    let held: Vec<bool> = panels
        .iter()
        .map(|panel| state.panel_is_open(panel.id))
        .collect();
    if let Some(index) = line_buttons(ui, &labels, &held)
        && let Some(panel) = panels.get(index)
    {
        state.set_panel_open(panel.id, !state.panel_is_open(panel.id));
    }
}

fn draw_notes(ui: &Ui, notes: &[MenuNote]) {
    for note in notes {
        let color = match note.tone {
            NoteTone::Plain => INK_MUTED,
            NoteTone::Warn => NOT_READY,
        };
        note_line(ui, &note.text, color);
    }
}

fn describe(result: Result<String, String>) -> String {
    match result {
        Ok(text) | Err(text) => text,
    }
}
