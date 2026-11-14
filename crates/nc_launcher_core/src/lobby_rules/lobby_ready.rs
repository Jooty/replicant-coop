use replicant_proto::{LOBBY_MODE_CONTINUE, LobbyReady};

#[derive(Debug, Clone, Copy)]
pub struct ReadyInputs {
    pub mode: u8,
    pub game_ok: bool,
    pub name_ok: bool,
    pub new_game_slot_free: bool,
    pub continue_slot: Option<u8>,
    pub is_guest: bool,
}

pub fn lobby_ready(inputs: ReadyInputs) -> LobbyReady {
    let new_game_ok = inputs.name_ok && inputs.new_game_slot_free;
    let continuing = inputs.mode == LOBBY_MODE_CONTINUE;
    let mode_ok = if continuing {
        inputs.continue_slot.is_some() || (inputs.is_guest && new_game_ok)
    } else {
        new_game_ok
    };
    LobbyReady {
        ready: inputs.game_ok && mode_ok,
        mode: inputs.mode,
        slot: if continuing {
            inputs.continue_slot
        } else {
            None
        },
    }
}
