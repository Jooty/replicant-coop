use nc_launcher::app::{RESTART_TAB_LOGS, RESTART_TAB_SETTINGS, Screen, Tab, event_log_line};
use nc_launcher::net::LauncherEvent;

#[test]
fn logs_belong_to_the_launcher() {
    for lobby_open in [false, true] {
        assert!(
            Tab::shown(lobby_open)
                .iter()
                .all(|(_, label)| *label != "Logs"),
            "the game page has no Logs tab"
        );
    }
    assert_eq!(Tab::restart_screen(Some(RESTART_TAB_LOGS)), Screen::Logs);
    assert_eq!(
        Tab::restart_screen(Some(RESTART_TAB_SETTINGS)),
        Screen::Game
    );
    assert_eq!(
        event_log_line(&LauncherEvent::GameLinked, "Fixture Game"),
        "Fixture Game: the game connected to the launcher"
    );
    assert_eq!(
        event_log_line(&LauncherEvent::Joining, "Fixture Game"),
        "joining the host...",
        "a line that does not come from a game names none"
    );
}

#[test]
fn the_settings_restart_key_opens_settings() {
    assert_eq!(
        Tab::from_restart_value(Some(RESTART_TAB_SETTINGS)),
        Tab::Settings
    );
}
