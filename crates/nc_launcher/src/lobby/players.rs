use replicant_proto::{GUEST_BODY, HOST_BODY, PartnerBody};

pub const PLAYER_TABLE_HEADERS: [&str; 4] = ["Player", "Role", "Character", "Status"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerRole {
    Host,
    Guest,
}

impl PlayerRole {
    pub fn text(self) -> &'static str {
        match self {
            Self::Host => "Host",
            Self::Guest => "Guest",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RowInputs<'a> {
    pub viewer_is_guest: bool,
    pub own_name: &'a str,
    pub peer_name: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerRow {
    pub name: String,
    pub role: PlayerRole,
    pub you: bool,
    pub body: PartnerBody,
}

pub fn player_rows(inputs: RowInputs) -> Vec<PlayerRow> {
    let (host_name, guest_name) = if inputs.viewer_is_guest {
        (inputs.peer_name, Some(inputs.own_name))
    } else {
        (Some(inputs.own_name), inputs.peer_name)
    };
    let mut rows = Vec::new();
    if let Some(name) = host_name {
        rows.push(PlayerRow {
            name: name.to_owned(),
            role: PlayerRole::Host,
            you: !inputs.viewer_is_guest,
            body: HOST_BODY,
        });
    }
    if let Some(name) = guest_name {
        rows.push(PlayerRow {
            name: name.to_owned(),
            role: PlayerRole::Guest,
            you: inputs.viewer_is_guest,
            body: GUEST_BODY,
        });
    }
    rows
}
