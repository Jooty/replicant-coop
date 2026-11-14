use eframe::egui::{Align2, FontId, Id, Painter, Rect, Stroke, Ui, pos2, vec2};
use nc_launcher_core::{ConsoleTask, ConsoleView, Easing, METER_FOLLOW_MS};

use crate::app::motion::ease_toward;
use crate::app::pages::game_title::game_title_job;
use crate::app::palette::{HAIRLINE_STRONG, INK, INK_MUTED};
use crate::app::parts::{
    DOUBLE_RULE_HEIGHT_PX, caps_galley, meter_width, paint_double_rule, paint_meter_cells,
    paint_square, spaced_job,
};
use crate::app::style::{CAPS_LETTER_SPACING_EM, caps_font, medium_family};
use crate::app::{Launcher, game_look};

pub(super) const CONSOLE_HEADING_HEIGHT_PX: f32 = 112.0;
const BAR_HEIGHT_PX: f32 = 60.0;
const TASK_SQUARE_SIDE_PX: f32 = 7.0;
const TASK_SQUARE_TO_TEXT_PX: f32 = 10.0;
const TASK_TO_DIVIDER_PX: f32 = 22.0;
const DIVIDER_HEIGHT_PX: f32 = 28.0;
const DIVIDER_TO_TITLE_PX: f32 = 22.0;
const BAR_GARAMOND_TITLE_PX: f32 = 28.0;
const BAR_CAPS_TITLE_PX: f32 = 22.0;
const RULE_TO_METER_PX: f32 = 22.0;
const METER_ROW_PX: f32 = 16.0;
const METER_TO_COUNT_PX: f32 = 18.0;
const METER_CELL_PITCH_PX: f32 = 17.0;
const HOSTING_LABEL: &str = "Hosting";
const JOINING_LABEL: &str = "Joining";
const STEPS_LABEL: &str = "Steps";
const METER_SALT: &str = "boot-console-meter";

pub(super) fn paint_console_heading(
    ui: &Ui,
    area: Rect,
    task: ConsoleTask,
    launcher: &Launcher,
    view: &ConsoleView,
    run_salt: u64,
) {
    let painter = ui.painter();
    let middle = area.top() + BAR_HEIGHT_PX / 2.0;
    paint_square(
        painter,
        pos2(area.left() + TASK_SQUARE_SIDE_PX / 2.0, middle),
        TASK_SQUARE_SIDE_PX,
        INK,
    );
    let task_label = match task {
        ConsoleTask::Hosting => HOSTING_LABEL,
        ConsoleTask::Joining => JOINING_LABEL,
    };
    let task_galley = painter.layout_job(spaced_job(
        &task_label.to_uppercase(),
        FontId::new(caps_font().size, medium_family()),
        INK,
        CAPS_LETTER_SPACING_EM,
    ));
    let task_left = area.left() + TASK_SQUARE_SIDE_PX + TASK_SQUARE_TO_TEXT_PX;
    let divider_x = task_left + task_galley.size().x + TASK_TO_DIVIDER_PX;
    painter.galley(
        pos2(task_left, middle - task_galley.size().y / 2.0),
        task_galley,
        INK,
    );
    painter.vline(
        divider_x + 0.5,
        middle - DIVIDER_HEIGHT_PX / 2.0..=middle + DIVIDER_HEIGHT_PX / 2.0,
        Stroke::new(1.0, HAIRLINE_STRONG),
    );
    let title = painter.layout_job(game_title_job(
        launcher.profile.display_name,
        game_look(launcher.profile).title_face,
        BAR_GARAMOND_TITLE_PX,
        BAR_CAPS_TITLE_PX,
        INK,
    ));
    painter.galley(
        pos2(
            divider_x + DIVIDER_TO_TITLE_PX,
            middle - title.size().y / 2.0,
        ),
        title,
        INK,
    );
    let rule_top = area.top() + BAR_HEIGHT_PX;
    paint_double_rule(painter, area.x_range(), rule_top);
    let meter_middle = rule_top + DOUBLE_RULE_HEIGHT_PX + RULE_TO_METER_PX + METER_ROW_PX / 2.0;
    paint_meter(ui, painter, area, meter_middle, view, run_salt);
}

fn paint_meter(
    ui: &Ui,
    painter: &Painter,
    area: Rect,
    middle: f32,
    view: &ConsoleView,
    run_salt: u64,
) {
    let total = view.counted_steps.max(1);
    let count = caps_galley(
        painter,
        &format!(
            "{STEPS_LABEL}  {:02} / {:02}",
            view.closed_steps, view.counted_steps
        ),
        INK_MUTED,
    );
    let count_at = Align2::RIGHT_CENTER
        .anchor_size(pos2(area.right(), middle), count.size())
        .min;
    painter.galley(count_at, count, INK_MUTED);
    let room = count_at.x - METER_TO_COUNT_PX - area.left();
    let cells = ((room + METER_CELL_PITCH_PX - meter_width(1)) / METER_CELL_PITCH_PX)
        .floor()
        .max(1.0) as u32;
    let share = view.closed_steps as f32 / total as f32;
    let shown = ease_toward(
        ui,
        Id::unique((METER_SALT, run_salt)),
        share,
        METER_FOLLOW_MS,
        Easing::EaseOut,
    );
    let filled = (shown * cells as f32).round() as u32;
    let rect = Rect::from_min_size(
        pos2(area.left(), middle - METER_ROW_PX / 2.0),
        vec2(meter_width(cells), METER_ROW_PX),
    );
    paint_meter_cells(painter, rect, cells, filled, INK);
}
