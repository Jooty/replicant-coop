use nc_wire::SessionRules;

#[derive(Debug, Clone, Copy)]
pub struct StartInputs<'a> {
    pub connected: bool,
    pub viewer_is_guest: bool,
    pub already_started: bool,
    pub own_ready: bool,
    pub others_ready: &'a [bool],
    pub rules: SessionRules,
}

/// The host may start once everyone is ready, at least one other player is in, and the whole
/// party fits the mode.
pub fn start_allowed(inputs: StartInputs) -> bool {
    let party = inputs.others_ready.len() + 1;
    inputs.connected
        && !inputs.viewer_is_guest
        && !inputs.already_started
        && inputs.own_ready
        && !inputs.others_ready.is_empty()
        && inputs.others_ready.iter().all(|ready| *ready)
        && party <= usize::from(inputs.rules.max_players)
}
