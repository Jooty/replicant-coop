use std::sync::atomic::{AtomicBool, Ordering};

use hudhook::imgui::{Context, Io, Key, Ui};
use hudhook::windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use hudhook::{BeforeWndProc, ImguiRenderLoop, MessageFilter, RenderContext};

use crate::build_gate::{GateError, VerifiedBuild};
use crate::hud_guard::HudGuard;
use crate::menu::GameMenu;
use crate::menu_view::{MenuState, draw_menu_window, watch_area_changes, with_menu_look};
use crate::overlay::OverlayGame;
use crate::overlay::extra_panels::draw_extra_panels;
use crate::overlay::style::apply_shared_style;
use crate::overlay::swapchain_window;

const TOGGLE_KEY: Key = Key::Insert;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_MOUSELAST: u32 = 0x0209;

pub(super) struct MenuOverlay<G> {
    visible: bool,
    cursor_mode: AtomicBool,
    gate: Result<VerifiedBuild, GateError>,
    game: G,
    state: MenuState,
    hud_guard: HudGuard,
}

impl<G: OverlayGame> MenuOverlay<G> {
    pub(super) fn new(game: G, gate: Result<VerifiedBuild, GateError>) -> Self {
        Self {
            visible: false,
            cursor_mode: AtomicBool::new(false),
            gate,
            game,
            state: MenuState::new(),
            hud_guard: HudGuard::default(),
        }
    }
}

impl<G: OverlayGame> ImguiRenderLoop for MenuOverlay<G> {
    fn initialize<'a>(&'a mut self, context: &mut Context, render: &'a mut dyn RenderContext) {
        apply_shared_style(context);
        self.game.initialize(context, render);
    }

    fn before_render<'a>(&'a mut self, context: &mut Context, _render: &'a mut dyn RenderContext) {
        let taking_mouse = self.game.prepare_input(context.io_mut(), self.visible);
        self.cursor_mode.store(taking_mouse, Ordering::Relaxed);
    }

    fn render(&mut self, ui: &mut Ui) {
        self.game.on_frame(ui);
        if ui.is_key_pressed_no_repeat(TOGGLE_KEY) {
            self.visible = !self.visible;
        }
        watch_area_changes(&self.game as &dyn GameMenu, &mut self.state);
        let game = &mut self.game;
        self.hud_guard.run(|| game.draw_hud(ui));
        if !self.visible {
            return;
        }
        // draw every ImGui frame: a frame without the windows leaves nothing hovered on the next
        // one, so a click landing in either frame is lost
        self.game.on_visible_frame(ui);
        with_menu_look(ui, || {
            draw_menu_window(
                ui,
                &mut self.game,
                |game, ui| game.draw_settings(ui),
                self.gate.as_ref().err(),
                &mut self.state,
            );
            draw_extra_panels(ui, &mut self.game, &mut self.state);
        });
    }

    fn before_wnd_proc(
        &self,
        hwnd: HWND,
        umsg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> BeforeWndProc {
        swapchain_window::remember(hwnd.0 as usize);
        if (WM_MOUSEMOVE..=WM_MOUSELAST).contains(&umsg) {
            swapchain_window::remember_client_position(lparam.0);
        }
        if self.game.observe_window_message(umsg, wparam.0, lparam.0) {
            return BeforeWndProc::Break;
        }
        BeforeWndProc::Continue
    }

    fn message_filter(&self, io: &Io) -> MessageFilter {
        if !self.visible {
            return MessageFilter::empty();
        }
        let mut filter = MessageFilter::empty();
        if self.cursor_mode.load(Ordering::Relaxed) {
            filter |= MessageFilter::InputMouse | MessageFilter::InputRaw;
        }
        if io.want_capture_mouse {
            filter |= MessageFilter::InputMouse;
        }
        if io.want_capture_keyboard {
            filter |= MessageFilter::InputKeyboard;
        }
        filter
    }
}
