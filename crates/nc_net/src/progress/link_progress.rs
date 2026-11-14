use std::net::IpAddr;
use std::time::Duration;

/// How far an address of this computer reaches: never the address itself, which is a player's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressReach {
    ThisComputer,
    LocalNetwork,
    Public,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddressKind {
    pub ipv6: bool,
    pub reach: AddressReach,
}

/// The way a path to the other player goes: straight to one of its addresses, or through a relay server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathRoute {
    Direct(AddressKind),
    Relayed,
}

/// How a joiner found the host from its key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookupMeans {
    LocalNetwork,
    PublishedRecord,
    TicketAddresses,
}

/// One thing the network did while a session was being set up, as the networking library or the session
/// reported it. Ids are the short form, relays are named by their server, and addresses are only counted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkProgress {
    Bound {
        ipv4_port: Option<u16>,
        ipv6_port: Option<u16>,
    },
    OwnId(String),
    RelayChosen {
        relay: String,
    },
    RelayAnswered {
        relay: String,
        after: Duration,
    },
    DirectAddressFound {
        kind: AddressKind,
        count: usize,
    },
    LocalLookupAnswering {
        port: u16,
    },
    LocalLookupOff {
        port: u16,
    },
    KeyPublished {
        after: Duration,
    },
    KeyNotPublished {
        waited: Duration,
    },
    TicketRead {
        host_id: String,
        relay: Option<String>,
        direct_addresses: usize,
    },
    LookingUpHost,
    HostFound {
        by: LookupMeans,
        after: Duration,
        relay: Option<String>,
        direct_addresses: usize,
    },
    HostNotFound {
        reason: String,
    },
    HostAnswered {
        after: Duration,
    },
    DialFailed,
    PathTried(PathRoute),
    PathWon {
        route: PathRoute,
        rtt: Option<Duration>,
    },
    HelloSent {
        protocol: u16,
    },
    WelcomeRead {
        seat: u8,
        host_name: String,
    },
}

impl AddressKind {
    pub fn of(ip: IpAddr) -> Self {
        let reach = match ip {
            IpAddr::V4(ip) if ip.is_loopback() => AddressReach::ThisComputer,
            IpAddr::V4(ip)
                if ip.is_private() || ip.is_link_local() || is_shared_v4(ip.octets()) =>
            {
                AddressReach::LocalNetwork
            }
            IpAddr::V6(ip) if ip.is_loopback() => AddressReach::ThisComputer,
            IpAddr::V6(ip) if ip.is_unique_local() || ip.is_unicast_link_local() => {
                AddressReach::LocalNetwork
            }
            _ => AddressReach::Public,
        };
        Self {
            ipv6: ip.is_ipv6(),
            reach,
        }
    }

    pub fn words(self) -> &'static str {
        match (self.ipv6, self.reach) {
            (false, AddressReach::ThisComputer) => "ipv4 this computer",
            (false, AddressReach::LocalNetwork) => "ipv4 local network",
            (false, AddressReach::Public) => "ipv4 public",
            (true, AddressReach::ThisComputer) => "ipv6 this computer",
            (true, AddressReach::LocalNetwork) => "ipv6 local network",
            (true, AddressReach::Public) => "ipv6 public",
        }
    }
}

impl LookupMeans {
    pub fn words(self) -> &'static str {
        match self {
            LookupMeans::LocalNetwork => "on the local network",
            LookupMeans::PublishedRecord => "by its published record",
            LookupMeans::TicketAddresses => "at the old ticket's addresses",
        }
    }
}

impl PathRoute {
    pub fn words(self) -> String {
        match self {
            PathRoute::Direct(kind) => format!("direct, {}", kind.words()),
            PathRoute::Relayed => "relayed".to_owned(),
        }
    }
}

impl LinkProgress {
    /// The progress as a sentence for the launcher's log.
    pub fn describe(&self) -> String {
        match self {
            LinkProgress::Bound {
                ipv4_port,
                ipv6_port,
            } => format!(
                "bound UDP port {} (ipv4), {} (ipv6)",
                port_words(*ipv4_port),
                port_words(*ipv6_port)
            ),
            LinkProgress::OwnId(id) => format!("own link id {id}"),
            LinkProgress::RelayChosen { relay } => format!("relay {relay} chosen"),
            LinkProgress::RelayAnswered { relay, after } => {
                format!("relay {relay} answered after {} ms", after.as_millis())
            }
            LinkProgress::DirectAddressFound { kind, count } => {
                format!("direct address {count} found ({})", kind.words())
            }
            LinkProgress::LocalLookupAnswering { port } => {
                format!("answering local network lookups on UDP port {port}")
            }
            LinkProgress::LocalLookupOff { port } => {
                format!("local network lookups not answered: UDP port {port} is in use")
            }
            LinkProgress::KeyPublished { after } => format!(
                "key published: the host can be looked up by its key after {} ms",
                after.as_millis()
            ),
            LinkProgress::KeyNotPublished { waited } => format!(
                "the key's published record did not show after {} s: only the local network can find this host",
                waited.as_secs()
            ),
            LinkProgress::TicketRead {
                host_id,
                relay: None,
                direct_addresses: 0,
            } => format!("ticket read: host key {host_id}"),
            LinkProgress::TicketRead {
                host_id,
                relay,
                direct_addresses,
            } => format!(
                "older ticket read: host {host_id}, relay {}, {direct_addresses} direct addresses",
                relay.as_deref().unwrap_or("none")
            ),
            LinkProgress::LookingUpHost => "looking up the host by its key".to_owned(),
            LinkProgress::HostFound {
                by,
                after,
                relay,
                direct_addresses,
            } => format!(
                "host found {} after {} ms: relay {}, {direct_addresses} direct addresses",
                by.words(),
                after.as_millis(),
                relay.as_deref().unwrap_or("none")
            ),
            LinkProgress::HostNotFound { reason } => {
                format!("the host's key was not found: {reason}")
            }
            LinkProgress::HostAnswered { after } => {
                format!("the host answered after {} ms", after.as_millis())
            }
            LinkProgress::DialFailed => "the host could not be reached".to_owned(),
            LinkProgress::PathTried(route) => format!("path tried: {}", route.words()),
            LinkProgress::PathWon { route, rtt } => match rtt {
                Some(rtt) => format!(
                    "path won: {}, round trip {} ms",
                    route.words(),
                    rtt.as_millis()
                ),
                None => format!("path won: {}", route.words()),
            },
            LinkProgress::HelloSent { protocol } => format!("hello sent (protocol {protocol})"),
            LinkProgress::WelcomeRead { seat, host_name } => format!(
                "welcome read from {host_name}: seat {:02}",
                u16::from(*seat) + 1
            ),
        }
    }
}

fn port_words(port: Option<u16>) -> String {
    port.map_or_else(|| "none".to_owned(), |port| port.to_string())
}

fn is_shared_v4(octets: [u8; 4]) -> bool {
    // 100.64.0.0/10, the carrier-grade NAT range.
    octets[0] == 100 && (octets[1] & 0xc0) == 64
}
