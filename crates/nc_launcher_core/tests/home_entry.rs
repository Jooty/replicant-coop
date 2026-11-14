use nc_launcher_core::{
    LOBBY_MODE_CONTINUE, LOBBY_MODE_NEW_GAME, SeatNameInputs, hosting_kind, own_seat_name,
    ticket_check,
};

fn seat_inputs(mode: u8, viewer_is_guest: bool) -> SeatNameInputs<'static> {
    SeatNameInputs {
        mode,
        viewer_is_guest,
        partner_seated: false,
        picked_save_name: Some("Okabe"),
        hosts_card_seat_name: Some("Mira"),
        typed_name: Some("Jay"),
        stored_name: "Stored",
    }
}

#[test]
fn hosting_opens_continue_when_a_coop_save_exists() {
    assert_eq!(hosting_kind(true), LOBBY_MODE_CONTINUE);
}

#[test]
fn hosting_opens_new_game_without_a_save() {
    assert_eq!(hosting_kind(false), LOBBY_MODE_NEW_GAME);
}

#[test]
fn the_seat_name_is_the_saves_name_on_continue() {
    let host = own_seat_name(seat_inputs(LOBBY_MODE_CONTINUE, false));
    assert_eq!(host.name, "Okabe");
    let guest = own_seat_name(seat_inputs(LOBBY_MODE_CONTINUE, true));
    assert_eq!(guest.name, "Mira");
    let without_card = own_seat_name(SeatNameInputs {
        hosts_card_seat_name: None,
        ..seat_inputs(LOBBY_MODE_CONTINUE, true)
    });
    assert_eq!(without_card.name, "Stored");
    let unnamed_save = own_seat_name(SeatNameInputs {
        picked_save_name: Some("  "),
        ..seat_inputs(LOBBY_MODE_CONTINUE, false)
    });
    assert_eq!(unnamed_save.name, "Stored");
}

#[test]
fn the_seat_name_can_be_edited_only_on_new_game() {
    assert!(!own_seat_name(seat_inputs(LOBBY_MODE_CONTINUE, false)).editable);
    assert!(!own_seat_name(seat_inputs(LOBBY_MODE_CONTINUE, true)).editable);
    let new_game = own_seat_name(seat_inputs(LOBBY_MODE_NEW_GAME, false));
    assert_eq!(new_game.name, "Jay");
    assert!(new_game.editable);
    let partner_in = own_seat_name(SeatNameInputs {
        partner_seated: true,
        ..seat_inputs(LOBBY_MODE_NEW_GAME, false)
    });
    assert!(!partner_in.editable);
    assert!(!own_seat_name(seat_inputs(LOBBY_MODE_NEW_GAME, true)).editable);
    let nothing_typed = own_seat_name(SeatNameInputs {
        typed_name: None,
        ..seat_inputs(LOBBY_MODE_NEW_GAME, false)
    });
    assert_eq!(nothing_typed.name, "Stored");
}

#[test]
fn a_ticket_that_is_no_address_is_refused_before_connecting() {
    assert!(ticket_check("").is_err());
    assert!(ticket_check("hello there").is_err());
    assert!(ticket_check("rcn1notbase32!").is_err());
}
