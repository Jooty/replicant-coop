use eframe::egui::{self, Event, Frame, Id, Key, Rect, Sense, Stroke, Ui, pos2};
use nc_launcher_core::{ConsoleTask, ConsoleView};

use crate::app::pages::console_band::{band_height_if_shown, draw_console_band_and_back};
use crate::app::pages::console_heading::{CONSOLE_HEADING_HEIGHT_PX, paint_console_heading};
use crate::app::pages::console_log::{console_log_height, paint_console_log_newest_in_view};
use crate::app::pages::lobby::session_open;
use crate::app::pages::tabs::{SIDE_GUTTER_PX, lobby_tab};
use crate::app::palette::{GROUND, SCAN_LINE};
use crate::app::parts::paint_grid_ground;
use crate::app::{JoinDialog, Launcher, NetForms, Screen, Tab, console_script};

const HEADING_TO_LOG_PX: f32 = 20.0;
const LOG_TO_BAND_PX: f32 = 18.0;
const PAGE_BOTTOM_GAP_PX: f32 = 24.0;
const SCAN_LINE_PITCH_PX: f32 = 3.0;
const CONSOLE_CLICK_SALT: &str = "boot-console-click";

pub(super) fn draw_boot_console(
    ui: &mut Ui,
    tab: &mut Tab,
    launcher: &mut Launcher,
    forms: &mut NetForms,
) {
    let Some(task) = forms.console.as_ref().map(|run| run.task) else {
        return;
    };
    let now_ms = ui.input(|input| input.time) * 1000.0;
    let script = console_script(launcher, forms, task, now_ms);
    let motion = launcher.config.motion;
    let Some(run) = forms.console.as_mut() else {
        return;
    };
    run.clock.observe(&script, now_ms);
    let run_salt = run.clock.started_ms().to_bits();
    let mut view = run.clock.view(&script, now_ms, motion);
    let (escape, other_key) = ui.input(|input| {
        let escape = input.key_pressed(Key::Escape);
        let other = input.events.iter().any(
            |event| matches!(event, Event::Key { key, pressed: true, .. } if *key != Key::Escape),
        );
        (escape, other)
    });
    let mut back = false;
    egui::CentralPanel::default()
        .frame(Frame::NONE.fill(GROUND))
        .show(ui, |ui| {
            let whole = ui.max_rect();
            let clicked = ui
                .interact(whole, Id::unique(CONSOLE_CLICK_SALT), Sense::click())
                .clicked();
            if view.failed {
                back = escape || clicked;
            } else if escape {
                back = true;
            } else if clicked || other_key {
                run.clock.skip();
                view = run.clock.view(&script, now_ms, motion);
            }
            if paint_console(ui, whole, task, launcher, &view, run_salt) {
                back = true;
            }
        });
    if back {
        forms.abandon();
        if task == ConsoleTask::Joining {
            forms.join_dialog = Some(JoinDialog::default());
        }
        return;
    }
    if view.lobby_due && session_open(forms) {
        forms.console = None;
        launcher.screen = Screen::Game;
        *tab = lobby_tab(launcher.lobby_mode);
        ui.ctx().request_repaint();
        return;
    }
    if view.moving || view.lobby_due || view.counting {
        ui.ctx().request_repaint();
    }
}

fn paint_console(
    ui: &mut Ui,
    whole: Rect,
    task: ConsoleTask,
    launcher: &Launcher,
    view: &ConsoleView,
    run_salt: u64,
) -> bool {
    paint_grid_ground(ui.painter(), whole);
    paint_scan_lines(ui, whole);
    let content = Rect::from_min_max(
        pos2(whole.left() + SIDE_GUTTER_PX, whole.top()),
        pos2(
            whole.right() - SIDE_GUTTER_PX,
            whole.bottom() - PAGE_BOTTOM_GAP_PX,
        ),
    );
    let heading = Rect::from_min_max(
        content.min,
        pos2(content.right(), content.top() + CONSOLE_HEADING_HEIGHT_PX),
    );
    paint_console_heading(ui, heading, task, launcher, view, run_salt);
    let log_top = heading.bottom() + HEADING_TO_LOG_PX;
    let band = band_height_if_shown(view);
    let log_bottom = match band {
        Some(height) => {
            let flowing = log_top + console_log_height(view);
            flowing.min(content.bottom() - height - LOG_TO_BAND_PX)
        }
        None => content.bottom(),
    };
    let log = Rect::from_min_max(
        pos2(content.left(), log_top),
        pos2(content.right(), log_bottom.max(log_top)),
    );
    paint_console_log_newest_in_view(ui, log, view, run_salt);
    let Some(height) = band else {
        return false;
    };
    let band_area = Rect::from_min_max(
        pos2(content.left(), log.bottom() + LOG_TO_BAND_PX),
        pos2(content.right(), log.bottom() + LOG_TO_BAND_PX + height),
    );
    draw_console_band_and_back(ui, band_area, view, run_salt)
}

fn paint_scan_lines(ui: &Ui, whole: Rect) {
    let painter = ui.painter();
    let stroke = Stroke::new(1.0, SCAN_LINE);
    let mut y = whole.top();
    while y < whole.bottom() {
        painter.hline(whole.x_range(), y + 0.5, stroke);
        y += SCAN_LINE_PITCH_PX;
    }
}
