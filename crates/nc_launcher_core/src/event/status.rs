use std::time::Duration;

use nc_wire::SessionRules;
use replicant_proto::{
    CampaignCard, GameReadiness, LobbyReady, PartnerBody, PlayerSettings, RejectReason,
    SeatRefusal, SessionOptions,
};

use crate::{STORY_RULES, SessionEndCause};

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum ConnectionState {
    #[default]
    Idle,
    Hosting,
    Joining,
    Connected,
    Rejected(RejectReason),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum LauncherEvent {
    Hosting {
        ticket: String,
    },
    Joining,
    MemberJoined {
        slot: u8,
        name: String,
    },
    MemberLeft {
        slot: u8,
        detail: String,
    },
    MemberChanged {
        slot: u8,
        change: MemberChange,
    },
    PeerRefused {
        reason: RejectReason,
    },
    Rejected {
        reason: RejectReason,
    },
    GameLinked,
    GameUnlinked {
        detail: String,
    },
    OwnGameReadiness {
        readiness: GameReadiness,
    },
    GameChangedSettings {
        settings: PlayerSettings,
    },
    SessionOptionsChanged {
        options: SessionOptions,
    },
    LobbyStartReceived {
        mode: u8,
    },
    GameSaved {
        succeeded: bool,
    },
    CampaignCardReceived {
        card: CampaignCard,
    },
    SeatReceived {
        slot: u8,
        transfer: u32,
        seat: Vec<u8>,
    },
    SeatDelivered {
        slot: u8,
        transfer: u32,
    },
    SeatTransferRefused {
        slot: u8,
        transfer: u32,
        reason: SeatRefusal,
        by_peer: bool,
    },
    SeatProgress {
        slot: u8,
        transfer: u32,
        parts_done: u16,
        parts: u16,
        incoming: bool,
    },
    MessageRefused {
        kind: String,
        reason: String,
    },
    /// What the network did while the session was set up, `after` the session was asked for.
    LinkProgress {
        after: Duration,
        line: String,
    },
    Failed {
        message: String,
    },
    /// A session ended; `last_seen` is the link as it was just before, seats included.
    SessionEnded {
        cause: SessionEndCause,
        last_seen: LinkStatus,
    },
    Stopped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberChange {
    Readiness(GameReadiness),
    Lobby(LobbyReady),
    Body(PartnerBody),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberStatus {
    pub slot: u8,
    pub name: String,
    pub readiness: GameReadiness,
    pub lobby: Option<LobbyReady>,
    pub body: Option<PartnerBody>,
    pub rtt_ms: Option<u32>,
}

impl MemberStatus {
    pub fn new(slot: u8, name: String) -> Self {
        Self {
            slot,
            name,
            readiness: GameReadiness::NotLinked,
            lobby: None,
            body: None,
            rtt_ms: None,
        }
    }

    pub fn apply(&mut self, change: MemberChange) {
        match change {
            MemberChange::Readiness(readiness) => self.readiness = readiness,
            MemberChange::Lobby(ready) => self.lobby = Some(ready),
            MemberChange::Body(body) => self.body = Some(body),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkStatus {
    pub state: ConnectionState,
    pub ticket: Option<String>,
    pub members: Vec<MemberStatus>,
    /// The viewer's own seat in the session: 0 for the host, the welcome's slot for a guest, `None` before it.
    pub own_slot: Option<u8>,
    pub rules: SessionRules,
    pub rtt: Option<Duration>,
    pub direct_path: Option<bool>,
    pub game_linked: bool,
    pub guest: bool,
    pub link_port: u16,
}

impl Default for LinkStatus {
    fn default() -> Self {
        Self {
            state: ConnectionState::default(),
            ticket: None,
            members: Vec::new(),
            own_slot: None,
            rules: STORY_RULES,
            rtt: None,
            direct_path: None,
            game_linked: false,
            guest: false,
            link_port: 0,
        }
    }
}

impl LinkStatus {
    pub fn partner(&self) -> Option<&MemberStatus> {
        match self.members.as_slice() {
            [only] => Some(only),
            _ => None,
        }
    }

    pub fn member(&self, slot: u8) -> Option<&MemberStatus> {
        self.members.iter().find(|member| member.slot == slot)
    }

    pub fn partner_readiness(&self) -> GameReadiness {
        self.partner()
            .map_or(GameReadiness::NotLinked, |partner| partner.readiness)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum NetState {
    #[default]
    Idle,
    Hosting {
        ticket: String,
    },
    Joining,
    Connected {
        peer_name: String,
    },
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NetStatus {
    pub state: NetState,
}
