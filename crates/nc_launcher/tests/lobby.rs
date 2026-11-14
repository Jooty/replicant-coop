use nc_launcher::lobby::{
    FollowNotice, GAME_QUIT_GRACE_SECONDS, GUEST_WAIT_FOR_HOST_GAME_SECONDS, GuestWaitStep,
    GuestWaiting, HostClosedInputs, LastLaunch, LaunchMode, MAX_NAME_BYTES, NameError,
    PLAYER_TABLE_HEADERS, PlayerRole, PlayerStatus, QuitFallback, QuitStep, ReadyInputs, RowInputs,
    guest_accepts_start, host_game_closed, host_options_editable, lobby_ready, new_game_slot,
    own_close_notice, player_rows, player_status, relaunch_choice, resolve_mode, start_enabled,
    validate_name,
};
use nc_launcher::net::{LOBBY_MODE_CONTINUE, LOBBY_MODE_NEW_GAME, LobbyReady};
use nc_launcher::saves::SlotSummary;
use replicant_proto::{GameReadiness, PartnerBody};

fn slot(index: u8, used: bool) -> SlotSummary {
    SlotSummary {
        index,
        used,
        name: String::new(),
        phase: String::new(),
        level: 1,
        play_seconds: 0.0,
    }
}

fn slots_with_used(used: &[u8]) -> Vec<SlotSummary> {
    (0..7)
        .map(|index| slot(index, used.contains(&index)))
        .collect()
}

fn ready(ready: bool, mode: u8) -> LobbyReady {
    LobbyReady {
        ready,
        mode,
        slot: None,
    }
}

fn inputs(mode: u8) -> ReadyInputs {
    ReadyInputs {
        mode,
        game_ok: true,
        name_ok: true,
        new_game_slot_free: true,
        continue_slot: None,
        is_guest: false,
    }
}

#[test]
fn a_name_is_trimmed_and_kept() {
    assert_eq!(validate_name("  Kit_2-b  ").unwrap(), "Kit_2-b");
}

#[test]
fn an_empty_name_is_refused() {
    assert_eq!(validate_name("   "), Err(NameError::Empty));
}

#[test]
fn a_name_over_the_limit_is_refused() {
    let longest = "a".repeat(MAX_NAME_BYTES);
    assert!(validate_name(&longest).is_ok());
    assert_eq!(
        validate_name(&format!("{longest}a")),
        Err(NameError::TooLong)
    );
}

#[test]
fn a_name_with_other_characters_is_refused() {
    assert_eq!(validate_name("Nier!"), Err(NameError::BadCharacter));
    assert_eq!(validate_name("Niér"), Err(NameError::BadCharacter));
}

#[test]
fn a_new_game_takes_the_first_empty_slot() {
    assert_eq!(new_game_slot(&slots_with_used(&[0, 1])), Some(2));
}

#[test]
fn a_new_game_with_no_save_file_takes_slot_zero() {
    assert_eq!(new_game_slot(&[]), Some(0));
}

#[test]
fn a_new_game_never_takes_the_seventh_slot() {
    assert_eq!(new_game_slot(&slots_with_used(&[0, 1, 2, 3, 4, 5])), None);
}

#[test]
fn only_the_host_can_start_and_only_when_both_are_ready() {
    let yes = ready(true, LOBBY_MODE_NEW_GAME);
    let no = ready(false, LOBBY_MODE_NEW_GAME);
    assert!(start_enabled(true, false, false, yes, Some(yes)));
    assert!(!start_enabled(true, false, false, yes, Some(no)));
    assert!(!start_enabled(true, false, false, no, Some(yes)));
    assert!(!start_enabled(true, false, false, yes, None));
    assert!(!start_enabled(true, true, false, yes, Some(yes)));
    assert!(!start_enabled(false, false, false, yes, Some(yes)));
}

#[test]
fn start_is_disabled_once_the_game_was_started() {
    let yes = ready(true, LOBBY_MODE_NEW_GAME);
    assert!(!start_enabled(true, false, true, yes, Some(yes)));
}

#[test]
fn a_guest_only_accepts_a_start_while_connected_ready_and_not_started() {
    let yes = ready(true, LOBBY_MODE_NEW_GAME);
    let no = ready(false, LOBBY_MODE_NEW_GAME);
    assert!(guest_accepts_start(true, true, false, yes));
    assert!(!guest_accepts_start(false, true, false, yes));
    assert!(!guest_accepts_start(true, true, true, yes));
    assert!(!guest_accepts_start(true, true, false, no));
    assert!(!guest_accepts_start(true, false, false, yes));
}

#[test]
fn a_new_game_needs_a_name_a_free_slot_and_the_game() {
    assert!(lobby_ready(inputs(LOBBY_MODE_NEW_GAME)).ready);
    let no_name = ReadyInputs {
        name_ok: false,
        ..inputs(LOBBY_MODE_NEW_GAME)
    };
    assert!(!lobby_ready(no_name).ready);
    let no_slot = ReadyInputs {
        new_game_slot_free: false,
        ..inputs(LOBBY_MODE_NEW_GAME)
    };
    assert!(!lobby_ready(no_slot).ready);
    let no_game = ReadyInputs {
        game_ok: false,
        ..inputs(LOBBY_MODE_NEW_GAME)
    };
    assert!(!lobby_ready(no_game).ready);
}

#[test]
fn the_host_continuing_needs_a_picked_slot() {
    let none = ReadyInputs {
        name_ok: false,
        ..inputs(LOBBY_MODE_CONTINUE)
    };
    assert!(!lobby_ready(none).ready);
    let picked = ReadyInputs {
        continue_slot: Some(3),
        ..none
    };
    let state = lobby_ready(picked);
    assert!(state.ready);
    assert_eq!(state.slot, Some(3));
}

#[test]
fn a_guest_without_a_save_can_still_join_a_continue_as_a_new_game() {
    let guest = ReadyInputs {
        is_guest: true,
        ..inputs(LOBBY_MODE_CONTINUE)
    };
    assert!(lobby_ready(guest).ready);
    let host = ReadyInputs {
        is_guest: false,
        ..guest
    };
    assert!(!lobby_ready(host).ready);
}

#[test]
fn continuing_uses_the_picked_slot() {
    assert_eq!(
        resolve_mode(LOBBY_MODE_CONTINUE, Some(2), Some("Kit".to_owned())),
        Some(LaunchMode::Continue { slot: 2 })
    );
}

#[test]
fn continuing_without_a_slot_starts_a_new_game_with_the_name() {
    assert_eq!(
        resolve_mode(LOBBY_MODE_CONTINUE, None, Some("Kit".to_owned())),
        Some(LaunchMode::NewGame {
            name: "Kit".to_owned()
        })
    );
    assert_eq!(resolve_mode(LOBBY_MODE_CONTINUE, None, None), None);
}

#[test]
fn a_new_game_ignores_the_picked_slot() {
    assert_eq!(
        resolve_mode(LOBBY_MODE_NEW_GAME, Some(2), Some("Kit".to_owned())),
        Some(LaunchMode::NewGame {
            name: "Kit".to_owned()
        })
    );
}

#[test]
fn a_guest_waits_while_the_hosts_game_is_not_linked() {
    let mut waiting = GuestWaiting::new(0);
    assert_eq!(
        waiting.step(GameReadiness::NotLinked, 10.0),
        GuestWaitStep::KeepWaiting
    );
    assert_eq!(
        waiting.step(GameReadiness::NotLinked, 50.0),
        GuestWaitStep::KeepWaiting
    );
}

#[test]
fn a_guest_launches_once_the_hosts_game_is_loading_or_later() {
    for readiness in [GameReadiness::Loading, GameReadiness::InPlay] {
        let mut waiting = GuestWaiting::new(1);
        assert_eq!(waiting.step(readiness, 5.0), GuestWaitStep::LaunchNow);
    }
}

#[test]
fn a_guest_gives_up_after_the_wait_limit() {
    let mut waiting = GuestWaiting::new(0);
    waiting.step(GameReadiness::NotLinked, 100.0);
    assert_eq!(
        waiting.step(
            GameReadiness::NotLinked,
            100.0 + GUEST_WAIT_FOR_HOST_GAME_SECONDS - 0.5
        ),
        GuestWaitStep::KeepWaiting
    );
    assert_eq!(
        waiting.step(
            GameReadiness::NotLinked,
            100.0 + GUEST_WAIT_FOR_HOST_GAME_SECONDS
        ),
        GuestWaitStep::GiveUp
    );
}

#[test]
fn a_host_game_that_appears_at_the_limit_still_wins() {
    let mut waiting = GuestWaiting::new(0);
    waiting.step(GameReadiness::NotLinked, 0.0);
    assert_eq!(
        waiting.step(
            GameReadiness::Loading,
            GUEST_WAIT_FOR_HOST_GAME_SECONDS + 5.0
        ),
        GuestWaitStep::LaunchNow
    );
}

#[test]
fn player_status_has_one_label_per_state() {
    assert_eq!(PlayerStatus::NotReady.text(), "Not ready");
    assert_eq!(PlayerStatus::Ready.text(), "Ready");
    assert_eq!(PlayerStatus::StartingGame.text(), "Starting game");
    assert_eq!(PlayerStatus::InGame.text(), "In game");
    assert_eq!(PlayerStatus::GameClosed.text(), "Game closed");
}

#[test]
fn player_status_reads_a_game_that_has_not_linked_from_the_lobby_ready_flag() {
    assert_eq!(
        player_status(false, GameReadiness::NotLinked, false),
        PlayerStatus::NotReady
    );
    assert_eq!(
        player_status(true, GameReadiness::NotLinked, false),
        PlayerStatus::Ready
    );
}

#[test]
fn player_status_reads_a_loading_game_as_starting_and_a_playing_game_as_in_game() {
    assert_eq!(
        player_status(true, GameReadiness::Loading, false),
        PlayerStatus::StartingGame
    );
    assert_eq!(
        player_status(false, GameReadiness::InPlay, false),
        PlayerStatus::InGame
    );
}

#[test]
fn player_status_priority_is_closed_then_in_game_then_starting_then_ready_then_not_ready() {
    for readiness in [
        GameReadiness::NotLinked,
        GameReadiness::Loading,
        GameReadiness::InPlay,
    ] {
        for ready in [false, true] {
            assert_eq!(
                player_status(ready, readiness, true),
                PlayerStatus::GameClosed
            );
        }
    }
    assert_eq!(
        player_status(true, GameReadiness::InPlay, false),
        PlayerStatus::InGame
    );
    assert_eq!(
        player_status(true, GameReadiness::Loading, false),
        PlayerStatus::StartingGame
    );
}

#[test]
fn player_status_labels_never_contain_parentheses() {
    for status in [
        PlayerStatus::NotReady,
        PlayerStatus::Ready,
        PlayerStatus::StartingGame,
        PlayerStatus::InGame,
        PlayerStatus::GameClosed,
    ] {
        assert!(!status.text().contains('('));
    }
}

#[test]
fn the_player_table_headers_are_player_role_character_status() {
    assert_eq!(
        PLAYER_TABLE_HEADERS,
        ["Player", "Role", "Character", "Status"]
    );
}

#[test]
fn hosting_alone_lists_only_the_host() {
    let rows = player_rows(RowInputs {
        viewer_is_guest: false,
        own_name: "Ada",
        peer_name: None,
    });
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].role, PlayerRole::Host);
    assert!(rows[0].you);
}

#[test]
fn the_host_always_has_the_first_body_and_the_guest_the_swapped_body() {
    for viewer_is_guest in [false, true] {
        let rows = player_rows(RowInputs {
            viewer_is_guest,
            own_name: "Ada",
            peer_name: Some("Bo"),
        });
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].role, PlayerRole::Host);
        assert_eq!(rows[0].body, PartnerBody::Nier);
        assert_eq!(rows[1].role, PlayerRole::Guest);
        assert_eq!(rows[1].body, PartnerBody::SeedKaineE);
        assert_eq!(rows[1].you, viewer_is_guest);
    }
}

#[test]
fn every_character_has_a_label_and_the_nier_variants_share_one() {
    use nc_launcher::lobby::body_label;
    use replicant_proto::PartnerBody;
    assert_eq!(body_label(PartnerBody::SeedKaineE), "Kainé");
    assert_eq!(
        body_label(PartnerBody::SeedNierY),
        body_label(PartnerBody::Nier)
    );
}

#[test]
fn the_normal_body_is_labelled_with_its_plain_name() {
    use nc_launcher::lobby::body_label;
    use replicant_proto::PartnerBody;
    assert_eq!(body_label(PartnerBody::Nier), "Nier");
    assert_eq!(
        body_label(PartnerBody::PrologueCharacter),
        body_label(PartnerBody::Nier)
    );
}

fn host_closed_inputs() -> HostClosedInputs {
    HostClosedInputs {
        guest: true,
        session_up: true,
        lobby_started: true,
        waiting_for_host_game: false,
        own_game_closed: false,
        partner_seen_linked: true,
        partner: GameReadiness::NotLinked,
        notice_shown: false,
    }
}

#[test]
fn a_guest_follows_a_host_game_that_closed() {
    assert!(host_game_closed(host_closed_inputs()));
}

#[test]
fn the_host_launcher_never_follows_the_guests_game() {
    let inputs = HostClosedInputs {
        guest: false,
        ..host_closed_inputs()
    };
    assert!(!host_game_closed(inputs));
}

#[test]
fn nobody_follows_a_host_game_after_the_peer_left() {
    let inputs = HostClosedInputs {
        session_up: false,
        lobby_started: false,
        partner_seen_linked: false,
        ..host_closed_inputs()
    };
    assert!(!host_game_closed(inputs));
}

#[test]
fn a_host_game_that_never_started_or_still_runs_is_not_followed() {
    let never_linked = HostClosedInputs {
        partner_seen_linked: false,
        ..host_closed_inputs()
    };
    let running = HostClosedInputs {
        partner: GameReadiness::InPlay,
        ..host_closed_inputs()
    };
    assert!(!host_game_closed(never_linked));
    assert!(!host_game_closed(running));
}

#[test]
fn a_host_close_is_followed_once() {
    let inputs = HostClosedInputs {
        notice_shown: true,
        ..host_closed_inputs()
    };
    assert!(!host_game_closed(inputs));
}

#[test]
fn a_guest_still_waiting_for_the_host_game_has_no_game_to_close() {
    let inputs = HostClosedInputs {
        waiting_for_host_game: true,
        ..host_closed_inputs()
    };
    assert!(!host_game_closed(inputs));
}

#[test]
fn a_game_that_closes_in_a_session_is_noticed_and_a_peer_leaving_is_not() {
    assert_eq!(
        own_close_notice(true, false),
        Some(FollowNotice::YourGameClosed)
    );
    assert_eq!(own_close_notice(false, false), None);
    assert_eq!(
        own_close_notice(true, true),
        Some(FollowNotice::HostGameClosed)
    );
}

#[test]
fn the_quit_fallback_ends_the_processes_once_after_the_grace_time() {
    let mut fallback = QuitFallback::new(100.0);
    assert_eq!(fallback.step(100.0), QuitStep::Waiting);
    assert_eq!(
        fallback.step(100.0 + GAME_QUIT_GRACE_SECONDS - 0.5),
        QuitStep::Waiting
    );
    assert_eq!(
        fallback.step(100.0 + GAME_QUIT_GRACE_SECONDS),
        QuitStep::EndProcesses
    );
    assert_eq!(
        fallback.step(100.0 + GAME_QUIT_GRACE_SECONDS + 1.0),
        QuitStep::Done
    );
}

#[test]
fn the_relaunch_repeats_the_last_launch_and_continues_a_new_games_slot_once_it_exists() {
    let new_game = LastLaunch {
        guest: true,
        mode: LOBBY_MODE_NEW_GAME,
        slot: Some(3),
    };
    assert_eq!(
        relaunch_choice(new_game, |slot| slot == 3),
        (LOBBY_MODE_CONTINUE, Some(3))
    );
    assert_eq!(
        relaunch_choice(new_game, |_| false),
        (LOBBY_MODE_NEW_GAME, None)
    );
    let continued = LastLaunch {
        guest: true,
        mode: LOBBY_MODE_CONTINUE,
        slot: Some(2),
    };
    assert_eq!(
        relaunch_choice(continued, |slot| slot == 2),
        (LOBBY_MODE_CONTINUE, Some(2))
    );
}

#[test]
fn host_edits_the_co_op_settings() {
    assert!(host_options_editable(false));
}

#[test]
fn guest_sees_the_co_op_settings_read_only() {
    assert!(!host_options_editable(true));
}
