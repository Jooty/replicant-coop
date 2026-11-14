#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemovalCause {
    KilledByHost,
    KilledByGuest,
    KilledLocally,
    HostRemoval,
    SilentRemoval,
    GuestOnlyInFightWindow,
    GameRemoval,
    ZoneUnload,
}

impl RemovalCause {
    pub const fn text(self) -> &'static str {
        match self {
            Self::KilledByHost => "killed by the host",
            Self::KilledByGuest => "killed by the guest's hit",
            Self::KilledLocally => "killed locally, no kill message from the host",
            Self::HostRemoval => "removed because the host removed it",
            Self::SilentRemoval => "silent removal of a hidden copy, no kill, no credit, no drop",
            Self::GuestOnlyInFightWindow => {
                "only you had it inside a fight window, no kill, no credit, no drop"
            }
            Self::GameRemoval => "removed by the game here, no kill seen",
            Self::ZoneUnload => "zone unload",
        }
    }
}
