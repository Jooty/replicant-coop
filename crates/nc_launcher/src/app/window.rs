use std::sync::Arc;

use eframe::egui;

use crate::app::{
    Launcher, NetForms, RESTART_TAB_ARGUMENT, RESTART_UPDATED_ARGUMENT, Screen, Tab, UpdateState,
    WINDOW_HEIGHT, WINDOW_MIN_HEIGHT, WINDOW_MIN_WIDTH, WINDOW_TITLE, WINDOW_WIDTH,
    apply_launcher_look, draw,
};
use crate::config::{LauncherConfig, config_path};
use crate::net::{LauncherCore, LauncherNet, LobbyReady, StubNet};
use nc_launcher_core::{LauncherConfig as CoreConfig, LauncherScale, REPLICANT, iroh_factory};
use nc_net::IrohConfig;
use replicant_proto::PartnerBody;

const APP_ID: &str = "nier-coop";
const EVENT_REPAINT_MILLIS: u64 = 250;

struct LauncherApp {
    launcher: Launcher,
    forms: NetForms,
    core: Option<Arc<LauncherCore>>,
    tab: Tab,
    name_sent_to_core: String,
    lobby_sent: Option<LobbyReady>,
    body_sent: Option<PartnerBody>,
    applied_scale: LauncherScale,
    window_resize_pending: bool,
}

impl LauncherApp {
    fn start() -> Self {
        let mut launcher = Launcher::load();
        let (tab, restart_screen, updated_to, restarted_into_tab) = restart_arguments();
        if restarted_into_tab {
            launcher.open_game(&REPLICANT);
            launcher.screen = restart_screen;
        }
        if let Some(short) = updated_to {
            launcher.updater.state = UpdateState::Updated(short);
        }
        let config = CoreConfig::new(&launcher.effective_player_name());
        let core = match LauncherCore::new(config, iroh_factory(IrohConfig::default())) {
            Ok(core) => Some(Arc::new(core)),
            Err(error) => {
                launcher
                    .log
                    .push(format!("networking could not start: {error}"));
                None
            }
        };
        let name_sent_to_core = launcher.effective_player_name();
        let session_options = launcher.config.session_options();
        if let Some(core) = &core {
            core.set_player_settings(launcher.player_settings());
            core.set_session_options(session_options);
        }
        let launcher_scale_at_start = launcher.config.scale;
        launcher.link_port = core.as_ref().map(|core| core.link_port());
        let net: Arc<dyn LauncherNet> = match &core {
            Some(core) => core.clone(),
            None => Arc::new(StubNet),
        };
        Self {
            launcher,
            forms: {
                let mut forms = NetForms::new(net);
                forms.session_options = session_options;
                forms
            },
            core,
            tab,
            name_sent_to_core,
            lobby_sent: None,
            body_sent: None,
            applied_scale: launcher_scale_at_start,
            window_resize_pending: false,
        }
    }
}

impl LauncherApp {
    fn follow_the_scale(&mut self, ctx: &egui::Context) {
        let scale = &mut self.launcher.config.scale;
        let before = *scale;
        let (bigger, smaller) = ctx.input_mut(scale_key_presses);
        if bigger {
            *scale = scale.larger();
        }
        if smaller {
            *scale = scale.smaller();
        }
        if *scale != before {
            self.launcher.save_config();
        }
        let wanted = self.launcher.config.scale;
        if wanted != self.applied_scale {
            ctx.set_zoom_factor(wanted.factor());
            self.applied_scale = wanted;
            self.window_resize_pending = true;
            ctx.request_repaint();
        } else if self.window_resize_pending {
            // The zoom is live from this frame on, so the sizes below are in zoomed points.
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(
                WINDOW_MIN_WIDTH,
                WINDOW_MIN_HEIGHT,
            )));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                WINDOW_WIDTH,
                WINDOW_HEIGHT,
            )));
            self.window_resize_pending = false;
        }
    }
}

impl eframe::App for LauncherApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.follow_the_scale(ui.ctx());
        let now = ui.input(|input| input.time);
        self.launcher.tick(now, ui.ctx());
        self.forms.poll();
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(EVENT_REPAINT_MILLIS));
        if let Some(core) = &self.core {
            let name = self.launcher.effective_player_name();
            if name != self.name_sent_to_core {
                core.set_player_name(&name);
                self.name_sent_to_core = name;
            }
            let events = core.drain_events();
            self.launcher.note_events(&events, &*self.forms.net);
            if !self.launcher.link.guest {
                self.forms.session_options = self.launcher.config.session_options();
            }
            self.launcher.link = core.link_status();
            self.launcher.follow_the_hosts_kind();
        }
        self.launcher.step_seat_round(&*self.forms.net, now);
        let ready = self.launcher.own_lobby_ready();
        if self.lobby_sent != Some(ready) {
            self.forms.net.send_lobby_ready(ready);
            self.lobby_sent = Some(ready);
        }
        if self.launcher.link.guest {
            let body = self.launcher.own_lobby_body();
            if self.body_sent != Some(body) {
                self.forms.net.send_lobby_body(body);
                self.body_sent = Some(body);
            }
        }
        if self.launcher.take_quit_request() {
            self.forms.net.ask_game_to_quit();
        }
        draw(ui, &mut self.tab, &mut self.launcher, &mut self.forms);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(core) = &self.core {
            core.shutdown();
        }
        self.launcher.release_game_link();
    }
}

fn restart_arguments() -> (Tab, Screen, Option<String>, bool) {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let value_of = |name: &str| {
        arguments
            .iter()
            .position(|argument| argument == name)
            .and_then(|index| arguments.get(index + 1))
            .cloned()
    };
    let tab = Tab::from_restart_value(value_of(RESTART_TAB_ARGUMENT).as_deref());
    (
        tab,
        Tab::restart_screen(value_of(RESTART_TAB_ARGUMENT).as_deref()),
        value_of(RESTART_UPDATED_ARGUMENT),
        value_of(RESTART_TAB_ARGUMENT).is_some(),
    )
}

fn scale_key_presses(input: &mut egui::InputState) -> (bool, bool) {
    let command = egui::Modifiers::COMMAND;
    let bigger = [egui::Key::Plus, egui::Key::Equals]
        .into_iter()
        .fold(false, |pressed, key| {
            input.consume_key(command, key) | pressed
        });
    let smaller = input.consume_key(command, egui::Key::Minus);
    (bigger, smaller)
}

fn stored_scale() -> LauncherScale {
    config_path()
        .map(|path| LauncherConfig::load(&path).scale)
        .unwrap_or_default()
}

pub fn run() -> eframe::Result {
    crate::config::migrate_app_folders();
    let scale = stored_scale();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([
                WINDOW_WIDTH * scale.factor(),
                WINDOW_HEIGHT * scale.factor(),
            ])
            .with_min_inner_size([
                WINDOW_MIN_WIDTH * scale.factor(),
                WINDOW_MIN_HEIGHT * scale.factor(),
            ])
            .with_app_id(APP_ID)
            .with_icon(window_icon()),
        ..Default::default()
    };
    eframe::run_native(
        WINDOW_TITLE,
        options,
        Box::new(|creation| {
            apply_launcher_look(&creation.egui_ctx);
            creation.egui_ctx.set_zoom_factor(scale.factor());
            creation
                .egui_ctx
                .options_mut(|options| options.zoom_with_keyboard = false);
            Ok(Box::new(LauncherApp::start()))
        }),
    )
}

fn window_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../../assets/icon.png")).unwrap_or_default()
}
