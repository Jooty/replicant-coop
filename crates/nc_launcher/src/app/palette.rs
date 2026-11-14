use eframe::egui::Color32;
use nc_ui_style as shared;

pub const GROUND: Color32 = from_shared(shared::GROUND);
pub const BAND: Color32 = from_shared(shared::BAND);
pub const INK: Color32 = from_shared(shared::INK);
pub const INK_MUTED: Color32 = from_shared(shared::INK_MUTED);
pub const INK_UNAVAILABLE: Color32 = from_shared(shared::INK_UNAVAILABLE);
pub const INK_ON_SAND: Color32 = from_shared(shared::INK_ON_SAND);
pub const INK_MUTED_ON_SAND: Color32 = from_shared(shared::INK_MUTED_ON_SAND);
pub const SAND: Color32 = from_shared(shared::SAND);
pub const HARD_SHADOW: Color32 = from_shared(shared::HARD_SHADOW);
pub const HAIRLINE: Color32 = from_shared(shared::HAIRLINE);
pub const HAIRLINE_STRONG: Color32 = from_shared(shared::HAIRLINE_STRONG);
pub const GRID_LINE: Color32 = from_shared(shared::GRID_LINE);
pub const HATCH_LINE: Color32 = from_shared(shared::HATCH_LINE);
pub const HATCH_LINE_DENSE: Color32 = from_shared(shared::HATCH_LINE_DENSE);
pub const HOVER_WASH: Color32 = from_shared(shared::HOVER_WASH);
pub const READY: Color32 = from_shared(shared::READY);
pub const NOT_READY: Color32 = from_shared(shared::NOT_READY);
pub const READY_ON_SAND: Color32 = from_shared(shared::READY_ON_SAND);
pub const NOT_READY_ON_SAND: Color32 = from_shared(shared::NOT_READY_ON_SAND);
pub const SCAN_LINE: Color32 = from_shared(shared::SCAN_LINE);

pub const fn from_shared(rgba: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied_const(rgba[0], rgba[1], rgba[2], rgba[3])
}
