use replicant_proto::SessionOptions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LobbyOption {
    NamesOverPlayers,
    Distance,
    Ping,
    PartyFrame,
    EnemyHealth,
    EnemyDamage,
}

/// Distance and ping are shown inside the name tag, so they mean nothing while names are off.
pub fn option_available(options: &SessionOptions, option: LobbyOption) -> bool {
    match option {
        LobbyOption::Distance | LobbyOption::Ping => options.show_names,
        LobbyOption::NamesOverPlayers
        | LobbyOption::PartyFrame
        | LobbyOption::EnemyHealth
        | LobbyOption::EnemyDamage => true,
    }
}
