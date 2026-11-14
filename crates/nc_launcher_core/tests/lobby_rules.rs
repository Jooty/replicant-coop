use nc_launcher_core::{
    CO_OP_CAMPAIGN, GameMode, LobbyOption, PlayerStatus, REPLICANT, SessionOptions, StartInputs,
    current_mode, lobby_permissions, option_available, rules_for_mode, start_allowed,
};

const UNKNOWN_MODE: u8 = 200;

fn inputs(others_ready: &[bool], max_players_of: GameMode) -> StartInputs<'_> {
    StartInputs {
        connected: true,
        viewer_is_guest: false,
        already_started: false,
        own_ready: true,
        others_ready,
        rules: max_players_of.rules(),
    }
}

#[test]
fn a_game_with_one_mode_lists_one_mode() {
    assert_eq!(REPLICANT.modes, &[CO_OP_CAMPAIGN]);
    assert_eq!(
        current_mode(&REPLICANT, CO_OP_CAMPAIGN.rules()),
        Some(&CO_OP_CAMPAIGN)
    );
}

#[test]
fn the_campaign_holds_two() {
    assert!(CO_OP_CAMPAIGN.is_campaign());
    assert_eq!(CO_OP_CAMPAIGN.max_players, 2);
}

#[test]
fn picking_the_campaign_gives_two_players() {
    let rules = rules_for_mode(&REPLICANT, CO_OP_CAMPAIGN.id).expect("the campaign is pickable");
    assert_eq!((rules.mode, rules.max_players), (CO_OP_CAMPAIGN.id, 2));
}

#[test]
fn an_unknown_mode_cannot_be_picked() {
    assert_eq!(rules_for_mode(&REPLICANT, UNKNOWN_MODE), None);
}

#[test]
fn the_campaign_starts_with_two_ready_players() {
    assert!(start_allowed(inputs(&[true], CO_OP_CAMPAIGN)));
    assert!(!start_allowed(inputs(&[false], CO_OP_CAMPAIGN)));
    assert!(!start_allowed(inputs(&[], CO_OP_CAMPAIGN)));
}

#[test]
fn the_campaign_cannot_start_with_three() {
    assert!(!start_allowed(inputs(&[true, true], CO_OP_CAMPAIGN)));
}

#[test]
fn only_the_host_starts() {
    let mut guest = inputs(&[true], CO_OP_CAMPAIGN);
    guest.viewer_is_guest = true;
    assert!(!start_allowed(guest));
    let mut started = inputs(&[true], CO_OP_CAMPAIGN);
    started.already_started = true;
    assert!(!start_allowed(started));
}

#[test]
fn a_guest_cannot_change_the_mode_or_the_options() {
    let guest = lobby_permissions(true);
    assert!(!guest.change_mode);
    assert!(!guest.change_options);
    let host = lobby_permissions(false);
    assert!(host.change_mode && host.change_options);
}

#[test]
fn ping_and_distance_are_unavailable_when_names_are_off() {
    let names_off = SessionOptions {
        show_names: false,
        ..SessionOptions::default()
    };
    assert!(!option_available(&names_off, LobbyOption::Distance));
    assert!(!option_available(&names_off, LobbyOption::Ping));
    assert!(option_available(&names_off, LobbyOption::PartyFrame));
    let names_on = SessionOptions {
        show_names: true,
        ..SessionOptions::default()
    };
    assert!(option_available(&names_on, LobbyOption::Distance));
    assert!(option_available(&names_on, LobbyOption::Ping));
}

#[test]
fn ready_and_in_game_are_fine_and_every_other_status_is_not() {
    assert!(PlayerStatus::Ready.is_fine());
    assert!(PlayerStatus::InGame.is_fine());
    assert!(!PlayerStatus::NotReady.is_fine());
    assert!(!PlayerStatus::StartingGame.is_fine());
    assert!(!PlayerStatus::GameClosed.is_fine());
}
