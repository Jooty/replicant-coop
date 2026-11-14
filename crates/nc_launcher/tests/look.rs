use eframe::egui::CornerRadius;
use nc_launcher::app::{WINDOW_TITLE, launcher_visuals};

#[test]
fn every_widget_state_has_square_corners() {
    let visuals = launcher_visuals();
    let widgets = &visuals.widgets;
    for state in [
        &widgets.noninteractive,
        &widgets.inactive,
        &widgets.hovered,
        &widgets.active,
        &widgets.open,
    ] {
        assert_eq!(state.corner_radius, CornerRadius::ZERO);
    }
    assert_eq!(visuals.window_corner_radius, CornerRadius::ZERO);
    assert_eq!(visuals.menu_corner_radius, CornerRadius::ZERO);
}

#[test]
fn the_window_shadow_is_a_hard_offset() {
    let shadow = launcher_visuals().window_shadow;
    assert_eq!(shadow.blur, 0);
    assert_ne!(shadow.offset, [0, 0]);
}

#[test]
fn the_launcher_is_called_nier_together() {
    assert_eq!(WINDOW_TITLE, "Nier Together");
}
