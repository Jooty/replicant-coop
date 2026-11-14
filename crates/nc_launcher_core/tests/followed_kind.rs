use nc_launcher_core::{
    LOBBY_MODE_CONTINUE, LOBBY_MODE_NEW_GAME, ReadyInputs, followed_lobby_kind, lobby_ready,
};

fn guest_inputs(mode: u8) -> ReadyInputs {
    ReadyInputs {
        mode,
        game_ok: true,
        name_ok: true,
        new_game_slot_free: true,
        continue_slot: None,
        is_guest: true,
    }
}

#[test]
fn a_guest_follows_the_hosts_kind_of_game() {
    for tab in [LOBBY_MODE_NEW_GAME, LOBBY_MODE_CONTINUE] {
        for host in [LOBBY_MODE_NEW_GAME, LOBBY_MODE_CONTINUE] {
            assert_eq!(followed_lobby_kind(tab, true, Some(host)), host);
        }
        assert_eq!(followed_lobby_kind(tab, true, None), tab);
        assert_eq!(
            followed_lobby_kind(tab, false, Some(LOBBY_MODE_CONTINUE)),
            tab
        );
    }
}

#[test]
fn a_guest_who_joined_from_the_other_tab_can_be_ready() {
    let kind = followed_lobby_kind(LOBBY_MODE_NEW_GAME, true, Some(LOBBY_MODE_CONTINUE));
    let seat = ReadyInputs {
        continue_slot: Some(0),
        new_game_slot_free: false,
        ..guest_inputs(kind)
    };
    let ready = lobby_ready(seat);
    assert!(ready.ready);
    assert_eq!(ready.mode, LOBBY_MODE_CONTINUE);
    assert_eq!(ready.slot, Some(0));

    let kind = followed_lobby_kind(LOBBY_MODE_CONTINUE, true, Some(LOBBY_MODE_NEW_GAME));
    let picked_a_local_save = ReadyInputs {
        continue_slot: Some(3),
        ..guest_inputs(kind)
    };
    let ready = lobby_ready(picked_a_local_save);
    assert!(ready.ready);
    assert_eq!(ready.mode, LOBBY_MODE_NEW_GAME);
    assert_eq!(ready.slot, None);
}
