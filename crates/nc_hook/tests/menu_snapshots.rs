use std::path::Path;

use imgui::{
    Condition, Context, DrawCmd, DrawCmdParams, DrawData, DrawVert, FontSource, TextureId, Ui,
    WindowFlags,
};
use nc_hook::build_gate::GateError;
use nc_hook::menu::{
    AreaChangeRefusal, AreaEntry, CoOpStatus, ExtraPanel, FixEntry, FixOutcome, GameMenu, MenuNote,
    PlayerLine, SectionKind, StoryJumpEntry,
};
use nc_hook::menu_view::parts::{
    MeterStep, choice_line, fold_heading, line_buttons, meter_line, note_line, option_line,
    panel_window, sub_heading, switch_line,
};
use nc_hook::menu_view::{
    MENU_GROUP_GAP_PX, MENU_WINDOW_SIZE_PX, MenuState, PANEL_COLUMN_LEFT_PX, add_menu_fonts,
    draw_menu_window, with_menu_look,
};
use nc_ui_style::{
    ControlLook, ENEMY_DAMAGE_METER_CELLS, ENEMY_HEALTH_METER_CELLS, FONT_SIZE_PX, INK_MUTED,
    OptionNesting, font_bytes,
};

const CANVAS_SIZE_PX: [usize; 2] = [1280, 720];
const TALL_CANVAS_SIZE_PX: [usize; 2] = [1280, 1200];
const TALL_MENU_HEIGHT_PX: f32 = 1160.0;
const BACKDROP_RGB: [f32; 3] = [52.0, 60.0, 56.0];
const SNAPSHOT_FOLDER: &str = "menu-snapshots";
const FONT_TEXTURE_ID: usize = 1;
const FRAME_SECONDS: f32 = 1.0 / 60.0;
const FRAMES_PER_SCENE: usize = 4;
const AWAY_POINTER_PX: [f32; 2] = [1200.0, 700.0];
const STORY_ROW_POINTER_PX: [f32; 2] = [200.0, 210.0];
const FIXTURE_AREA_COUNT: u32 = 14;
const FIXTURE_CURRENT_AREA: u32 = 3;
const FIXTURE_STORY_JUMPS: u32 = 9;
const FIXTURE_PING_MS: u32 = 38;
const MAX_FIXTURE_HEALTH_PERCENT: i32 = 300;
const MAX_FIXTURE_DAMAGE_PERCENT: i32 = 50;
const FIXTURE_PANEL_ID: u32 = 1;

struct FixtureGame;

struct Scene {
    name: &'static str,
    open_sections: &'static [SectionKind],
    pointer: [f32; 2],
    host: bool,
    gate_failed: bool,
    panel: bool,
    tall: bool,
}

struct FontTexture {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}

struct Canvas {
    width: usize,
    height: usize,
    rgb: Vec<[f32; 3]>,
}

impl GameMenu for FixtureGame {
    fn co_op_status(&self) -> CoOpStatus {
        CoOpStatus {
            link: "127.0.0.1:47811, connected".to_owned(),
            connected: true,
            players: vec![
                PlayerLine {
                    name: "you".to_owned(),
                    detail: "phase 12".to_owned(),
                },
                PlayerLine {
                    name: "Mira".to_owned(),
                    detail: "phase 12, hp 210 / 240, distance: 3.4, last state 16 ms ago"
                        .to_owned(),
                },
            ],
            ping_ms: Some(FIXTURE_PING_MS),
        }
    }

    fn areas(&self) -> Vec<AreaEntry> {
        (0..FIXTURE_AREA_COUNT).map(area).collect()
    }

    fn current_area(&self) -> Option<AreaEntry> {
        Some(area(FIXTURE_CURRENT_AREA))
    }

    fn area_change_blocker(&self) -> Option<AreaChangeRefusal> {
        None
    }

    fn change_area(&self, _area: &AreaEntry) -> Result<(), String> {
        Ok(())
    }

    fn go_to_partner(&self) -> Result<String, String> {
        Ok(String::new())
    }

    fn bring_partner(&self) -> Result<String, String> {
        Ok(String::new())
    }

    fn story_jumps(&self) -> Vec<StoryJumpEntry> {
        (1..=FIXTURE_STORY_JUMPS)
            .map(|id| StoryJumpEntry {
                id,
                label: format!("Story point {id:02}"),
            })
            .collect()
    }

    fn run_story_jump(&self, _id: u32) -> Result<String, String> {
        Ok(String::new())
    }

    fn fixes(&self) -> Vec<FixEntry> {
        [
            "go to partner",
            "resync story",
            "rejoin the host's area",
            "respawn partner",
            "copy logs",
        ]
        .iter()
        .enumerate()
        .map(|(index, label)| FixEntry {
            id: u32::try_from(index).unwrap_or(u32::MAX),
            label: (*label).to_owned(),
            disabled_reason: (index == 2).then(|| "only a guest can rejoin".to_owned()),
        })
        .collect()
    }

    fn run_fix(&self, _id: u32) -> FixOutcome {
        FixOutcome::Done
    }

    fn status_lines(&self) -> Vec<String> {
        vec![
            "phase: 12".to_owned(),
            "position: 12.40 3.10 -88.02".to_owned(),
            "hp: 210 / 240".to_owned(),
            "mp: 100.0 / 100.0".to_owned(),
            "level: 31".to_owned(),
        ]
    }

    fn extra_panels(&self) -> Vec<ExtraPanel> {
        ["area point", "trainer", "diagnostics", "co-op details"]
            .iter()
            .enumerate()
            .map(|(index, label)| ExtraPanel {
                id: u32::try_from(index).unwrap_or(u32::MAX),
                label,
            })
            .collect()
    }

    fn section_notes(&self, section: SectionKind) -> Vec<MenuNote> {
        match section {
            SectionKind::CoOp => vec![
                MenuNote::plain("host and join live in the launcher"),
                MenuNote::plain("path: direct"),
            ],
            SectionKind::Fixes => vec![MenuNote::warn("you are far from the other player")],
            _ => Vec::new(),
        }
    }
}

impl FontTexture {
    fn sample(&self, uv: [f32; 2]) -> [f32; 4] {
        let x = (uv[0] * self.width as f32 - 0.5).max(0.0);
        let y = (uv[1] * self.height as f32 - 0.5).max(0.0);
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let texel = |tx: usize, ty: usize| {
            let at = (ty.min(self.height - 1) * self.width + tx.min(self.width - 1)) * 4;
            [0, 1, 2, 3].map(|channel| f32::from(self.rgba[at + channel]) / 255.0)
        };
        let (a, b, c, d) = (
            texel(x0, y0),
            texel(x0 + 1, y0),
            texel(x0, y0 + 1),
            texel(x0 + 1, y0 + 1),
        );
        [0, 1, 2, 3].map(|channel| {
            let top = a[channel] + (b[channel] - a[channel]) * fx;
            let bottom = c[channel] + (d[channel] - c[channel]) * fx;
            top + (bottom - top) * fy
        })
    }
}

impl Canvas {
    fn new(size: [usize; 2]) -> Self {
        Self {
            width: size[0],
            height: size[1],
            rgb: vec![BACKDROP_RGB; size[0] * size[1]],
        }
    }

    fn draw(&mut self, draw_data: &DrawData, font: &FontTexture) {
        for list in draw_data.draw_lists() {
            let vertices = list.vtx_buffer();
            let indices = list.idx_buffer();
            for command in list.commands() {
                let DrawCmd::Elements {
                    count,
                    cmd_params:
                        DrawCmdParams {
                            clip_rect,
                            texture_id,
                            vtx_offset,
                            idx_offset,
                        },
                } = command
                else {
                    continue;
                };
                let textured = texture_id.id() == FONT_TEXTURE_ID;
                for triangle in indices[idx_offset..idx_offset + count].as_chunks::<3>().0 {
                    let corner = |slot: usize| vertices[vtx_offset + usize::from(triangle[slot])];
                    self.fill_triangle(
                        [corner(0), corner(1), corner(2)],
                        clip_rect,
                        textured.then_some(font),
                    );
                }
            }
        }
    }

    fn fill_triangle(
        &mut self,
        corners: [DrawVert; 3],
        clip: [f32; 4],
        font: Option<&FontTexture>,
    ) {
        let [a, mut b, mut c] = corners;
        let mut area = edge(a.pos, b.pos, c.pos);
        if area == 0.0 {
            return;
        }
        if area < 0.0 {
            std::mem::swap(&mut b, &mut c);
            area = -area;
        }
        let xs = [a.pos[0], b.pos[0], c.pos[0]];
        let ys = [a.pos[1], b.pos[1], c.pos[1]];
        let left = xs
            .iter()
            .copied()
            .fold(f32::MAX, f32::min)
            .max(clip[0])
            .max(0.0);
        let right = xs
            .iter()
            .copied()
            .fold(f32::MIN, f32::max)
            .min(clip[2])
            .min(self.width as f32);
        let top = ys
            .iter()
            .copied()
            .fold(f32::MAX, f32::min)
            .max(clip[1])
            .max(0.0);
        let bottom = ys
            .iter()
            .copied()
            .fold(f32::MIN, f32::max)
            .min(clip[3])
            .min(self.height as f32);
        if left >= right || top >= bottom {
            return;
        }
        for py in top.floor() as usize..bottom.ceil() as usize {
            for px in left.floor() as usize..right.ceil() as usize {
                let point = [px as f32 + 0.5, py as f32 + 0.5];
                if point[0] < clip[0]
                    || point[0] >= clip[2]
                    || point[1] < clip[1]
                    || point[1] >= clip[3]
                {
                    continue;
                }
                let weights = [
                    edge(b.pos, c.pos, point),
                    edge(c.pos, a.pos, point),
                    edge(a.pos, b.pos, point),
                ];
                let edges = [(b.pos, c.pos), (c.pos, a.pos), (a.pos, b.pos)];
                let inside = weights.iter().zip(edges).all(|(weight, (from, to))| {
                    *weight > 0.0 || (*weight == 0.0 && owns_edge(from, to))
                });
                if !inside {
                    continue;
                }
                let [wa, wb, wc] = weights.map(|weight| weight / area);
                let mix = |pick: fn(&DrawVert, usize) -> f32, channel: usize| {
                    pick(&a, channel) * wa + pick(&b, channel) * wb + pick(&c, channel) * wc
                };
                let mut color =
                    [0, 1, 2, 3].map(|channel| mix(|v, i| f32::from(v.col[i]) / 255.0, channel));
                if let Some(font) = font {
                    let texel = font.sample([mix(|v, i| v.uv[i], 0), mix(|v, i| v.uv[i], 1)]);
                    color = [0, 1, 2, 3].map(|channel| color[channel] * texel[channel]);
                }
                let pixel = &mut self.rgb[py * self.width + px];
                for channel in 0..3 {
                    pixel[channel] =
                        color[channel] * 255.0 * color[3] + pixel[channel] * (1.0 - color[3]);
                }
            }
        }
    }

    fn save(&self, name: &str) {
        let mut image = image::RgbImage::new(self.width as u32, self.height as u32);
        for (index, pixel) in self.rgb.iter().enumerate() {
            let at = (index % self.width) as u32;
            let row = (index / self.width) as u32;
            image.put_pixel(
                at,
                row,
                image::Rgb(pixel.map(|channel| channel.round().clamp(0.0, 255.0) as u8)),
            );
        }
        let folder = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(SNAPSHOT_FOLDER);
        std::fs::create_dir_all(&folder).expect("snapshot folder");
        image
            .save(folder.join(format!("{name}.png")))
            .expect("png written");
    }
}

#[test]
fn the_menu_renders_in_the_launcher_look() {
    let scenes = [
        Scene {
            name: "menu-as-opened",
            open_sections: &[SectionKind::CoOp],
            pointer: AWAY_POINTER_PX,
            host: true,
            gate_failed: false,
            panel: false,
            tall: false,
        },
        Scene {
            name: "menu-teleport",
            open_sections: &[SectionKind::Teleport],
            pointer: AWAY_POINTER_PX,
            host: true,
            gate_failed: false,
            panel: false,
            tall: false,
        },
        Scene {
            name: "menu-story-jump-pointed",
            open_sections: &[SectionKind::StoryJump, SectionKind::Fixes],
            pointer: STORY_ROW_POINTER_PX,
            host: true,
            gate_failed: false,
            panel: false,
            tall: false,
        },
        Scene {
            name: "menu-settings-host",
            open_sections: &[SectionKind::SettingsAndLogs],
            pointer: AWAY_POINTER_PX,
            host: true,
            gate_failed: false,
            panel: true,
            tall: true,
        },
        Scene {
            name: "menu-settings-guest",
            open_sections: &[SectionKind::SettingsAndLogs],
            pointer: AWAY_POINTER_PX,
            host: false,
            gate_failed: true,
            panel: false,
            tall: true,
        },
    ];
    let mut context = Context::create();
    context.set_ini_filename(None::<std::path::PathBuf>);
    context.fonts().add_font(&[FontSource::TtfData {
        data: font_bytes(),
        size_pixels: FONT_SIZE_PX,
        config: None,
    }]);
    add_menu_fonts(&mut context);
    let font = {
        let atlas = context.fonts();
        let texture = atlas.build_rgba32_texture();
        FontTexture {
            width: texture.width as usize,
            height: texture.height as usize,
            rgba: texture.data.to_vec(),
        }
    };
    context.fonts().tex_id = TextureId::new(FONT_TEXTURE_ID);
    for scene in &scenes {
        render_scene(&mut context, &font, scene);
    }
}

fn render_scene(context: &mut Context, font: &FontTexture, scene: &Scene) {
    let mut state = MenuState::new();
    state.set_section_open(SectionKind::CoOp, false);
    for section in scene.open_sections {
        state.set_section_open(*section, true);
    }
    if scene.panel {
        state.set_panel_open(FIXTURE_PANEL_ID, true);
    }
    let gate_error = scene.gate_failed.then_some(GateError::NoMainModule);
    let mut game = FixtureGame;
    let canvas_size = if scene.tall {
        TALL_CANVAS_SIZE_PX
    } else {
        CANVAS_SIZE_PX
    };
    let mut canvas = Canvas::new(canvas_size);
    for frame in 0..FRAMES_PER_SCENE {
        let io = context.io_mut();
        io.display_size = [canvas_size[0] as f32, canvas_size[1] as f32];
        io.delta_time = FRAME_SECONDS;
        io.mouse_pos = scene.pointer;
        let ui = context.new_frame();
        let menu_size = if scene.tall {
            [MENU_WINDOW_SIZE_PX[0], TALL_MENU_HEIGHT_PX]
        } else {
            MENU_WINDOW_SIZE_PX
        };
        with_menu_look(ui, || {
            // the menu window takes its size only on first use; a first Begin this frame sets it here
            ui.window("menu##debug")
                .size(menu_size, Condition::Always)
                .flags(WindowFlags::NO_TITLE_BAR | WindowFlags::NO_COLLAPSE)
                .build(|| {});
            draw_menu_window(
                ui,
                &mut game,
                |_, ui| draw_fixture_settings(ui, scene.host),
                gate_error.as_ref(),
                &mut state,
            );
            let mut open = state.panel_is_open(FIXTURE_PANEL_ID);
            panel_window(
                ui,
                "trainer",
                &mut open,
                [PANEL_COLUMN_LEFT_PX, 16.0],
                [400.0, 440.0],
                || {
                    draw_fixture_panel(ui);
                },
            );
        });
        let draw_data = context.render();
        if frame + 1 == FRAMES_PER_SCENE {
            canvas.draw(draw_data, font);
        }
    }
    canvas.save(scene.name);
}

fn draw_fixture_settings(ui: &Ui, host: bool) {
    let mut keep_running = true;
    let mut event_log = true;
    switch_line(ui, "Keep running in the background", &mut keep_running);
    switch_line(ui, "Show the event log", &mut event_log);
    ui.dummy([0.0, MENU_GROUP_GAP_PX]);
    let look = if host {
        ControlLook::Editable
    } else {
        ControlLook::ReadOnly
    };
    let lines = [
        ("Show names over players", true, OptionNesting::Top),
        ("Show distance", false, OptionNesting::Branch),
        ("Show ping", true, OptionNesting::LastBranch),
        ("Show party frame", true, OptionNesting::Top),
    ];
    for (label, mut on, nesting) in lines {
        option_line(ui, label, &mut on, look, nesting);
    }
    let mut health = 60;
    let mut damage = 10;
    meter_line(
        ui,
        "Enemy health per extra player",
        &mut health,
        (0, MAX_FIXTURE_HEALTH_PERCENT),
        ENEMY_HEALTH_METER_CELLS,
        MeterStep::Cells,
        " %",
        look,
    );
    meter_line(
        ui,
        "Enemy damage per extra player",
        &mut damage,
        (0, MAX_FIXTURE_DAMAGE_PERCENT),
        ENEMY_DAMAGE_METER_CELLS,
        MeterStep::Cells,
        " %",
        look,
    );
    let mut hittable = false;
    option_line(
        ui,
        "Paused players can be hit",
        &mut hittable,
        look,
        OptionNesting::Top,
    );
    if !host {
        note_line(ui, "These are the host's choice.", INK_MUTED);
    }
    note_line(
        ui,
        "Shared with the launcher: change them here or there.",
        INK_MUTED,
    );
}

fn draw_fixture_panel(ui: &Ui) {
    let mut invincible = true;
    let mut infinite_magic = false;
    let mut noclip = false;
    switch_line(ui, "invincible (hp never drops)", &mut invincible);
    switch_line(ui, "infinite mp", &mut infinite_magic);
    switch_line(
        ui,
        "noclip (WASD, Space up, Ctrl down, Shift fast)",
        &mut noclip,
    );
    line_buttons(ui, &["refill hp", "refill mp", "+1000 gold"], &[]);
    sub_heading(ui, "stay together");
    choice_line(ui, "Name tag sync:", &["old", "new"], Some(1));
    let mut leash = 120;
    meter_line(
        ui,
        "leash (units)",
        &mut leash,
        (5, 500),
        12,
        MeterStep::Exact,
        "",
        ControlLook::Editable,
    );
    let mut folded = false;
    fold_heading(ui, "party frame layout", &mut folded);
    ui.text("queued RefillHealth");
}

fn area(index: u32) -> AreaEntry {
    AreaEntry {
        id: index,
        name: format!("Phase {index:02}  (p{index:02})"),
    }
}

fn edge(from: [f32; 2], to: [f32; 2], point: [f32; 2]) -> f32 {
    (to[0] - from[0]) * (point[1] - from[1]) - (to[1] - from[1]) * (point[0] - from[0])
}

fn owns_edge(from: [f32; 2], to: [f32; 2]) -> bool {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    dy > 0.0 || (dy == 0.0 && dx < 0.0)
}
