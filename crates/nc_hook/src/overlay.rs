mod extra_panels;
mod install;
mod overlay_game;
mod render_loop;
pub mod style;
pub mod swapchain_window;

pub use install::{OverlayConfig, install_when_renderer_ready};
pub use overlay_game::OverlayGame;

const RENDERER_POLL_INTERVAL_MS: u32 = 100;
const RENDERER_POLL_ATTEMPTS: u32 = 300;
const SETTLE_AFTER_RENDERER_LOADED_MS: u32 = 1000;
