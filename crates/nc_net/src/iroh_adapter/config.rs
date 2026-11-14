use crate::iroh_adapter::LOCAL_LOOKUP_PORT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrohConfig {
    pub use_relays_and_lookup: bool,
    /// The UDP port a host answers local network lookups on, and a joiner asks on.
    pub local_lookup_port: u16,
}

impl Default for IrohConfig {
    fn default() -> Self {
        Self {
            use_relays_and_lookup: true,
            local_lookup_port: LOCAL_LOOKUP_PORT,
        }
    }
}
