use imgui::ImColor32;

pub fn ink(rgba: [u8; 4]) -> ImColor32 {
    ImColor32::from_rgba(rgba[0], rgba[1], rgba[2], rgba[3])
}
