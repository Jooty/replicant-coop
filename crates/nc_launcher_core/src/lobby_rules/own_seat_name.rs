use replicant_proto::LOBBY_MODE_CONTINUE;

/// What the own seat's name is decided from.
#[derive(Debug, Clone, Copy)]
pub struct SeatNameInputs<'a> {
    pub mode: u8,
    pub viewer_is_guest: bool,
    pub partner_seated: bool,
    pub picked_save_name: Option<&'a str>,
    pub hosts_card_seat_name: Option<&'a str>,
    pub typed_name: Option<&'a str>,
    pub stored_name: &'a str,
}

/// The name on the own seat, which is also the one the session and the game get, and whether it may be
/// typed into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeatNameChoice {
    pub name: String,
    pub editable: bool,
}

/// On a saved game the name is the save's: the picked save for the host, the host's campaign card for a
/// guest, the stored name when neither carries one. On a new game the host types it until a partner sits
/// down, since the partner has been told the name by then.
pub fn own_seat_name(inputs: SeatNameInputs) -> SeatNameChoice {
    let named = |name: Option<&str>| {
        name.map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(inputs.stored_name)
            .to_owned()
    };
    if inputs.mode == LOBBY_MODE_CONTINUE {
        let save_name = if inputs.viewer_is_guest {
            inputs.hosts_card_seat_name
        } else {
            inputs.picked_save_name
        };
        return SeatNameChoice {
            name: named(save_name),
            editable: false,
        };
    }
    SeatNameChoice {
        name: named(inputs.typed_name),
        editable: !inputs.viewer_is_guest && !inputs.partner_seated,
    }
}
