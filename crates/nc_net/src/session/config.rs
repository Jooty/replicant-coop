use nc_wire::{BuildGate, SessionRules};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Host,
    Guest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionConfig {
    pub role: Role,
    pub name: String,
    pub build: BuildGate,
    pub rules: SessionRules,
}
