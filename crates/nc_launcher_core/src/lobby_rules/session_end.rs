use nc_net::LeaveReason;
use replicant_proto::RejectReason;

use crate::{HOST_UNREACHABLE_TEXT, judge_rejection_text};

const HOST_LEFT_TEXT: &str = "The host left the lobby.";
const LINK_LOST_TEXT: &str = "The link to the host was lost.";

/// Why a session ended. The first three are this launcher's own doing; the rest come from the other side
/// or the network.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionEndCause {
    PlayerLeft,
    GaveUp,
    Replaced,
    HostLeft,
    RemovedByHost,
    Refused(RejectReason),
    LinkLost,
    HostUnreachable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionEndSide {
    ThisPlayer,
    OtherPlayer,
    Network,
}

/// Where the launcher goes when a session ends: home, nowhere, or the lobby page kept with what happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfterSessionEnd {
    Home,
    StayPut,
    EndedLobby,
}

impl SessionEndCause {
    /// How a guest's session ended when the host's seat went away for `reason`.
    pub fn from_host_leaving(reason: LeaveReason) -> Self {
        match reason {
            LeaveReason::Quit => SessionEndCause::HostLeft,
            LeaveReason::RemovedByHost => SessionEndCause::RemovedByHost,
            LeaveReason::TimedOut
            | LeaveReason::ProtocolViolation
            | LeaveReason::TransportClosed => SessionEndCause::LinkLost,
        }
    }

    pub fn side(self) -> SessionEndSide {
        match self {
            SessionEndCause::PlayerLeft | SessionEndCause::GaveUp | SessionEndCause::Replaced => {
                SessionEndSide::ThisPlayer
            }
            SessionEndCause::HostLeft
            | SessionEndCause::RemovedByHost
            | SessionEndCause::Refused(_) => SessionEndSide::OtherPlayer,
            SessionEndCause::LinkLost | SessionEndCause::HostUnreachable => SessionEndSide::Network,
        }
    }

    /// The one line the player is shown for an end they did not ask for; none for their own.
    pub fn reason_line(self) -> Option<&'static str> {
        match self {
            SessionEndCause::PlayerLeft | SessionEndCause::GaveUp | SessionEndCause::Replaced => {
                None
            }
            SessionEndCause::HostLeft => Some(HOST_LEFT_TEXT),
            SessionEndCause::RemovedByHost => {
                Some(judge_rejection_text(RejectReason::RemovedByHost))
            }
            SessionEndCause::Refused(reason) => Some(judge_rejection_text(reason)),
            SessionEndCause::LinkLost => Some(LINK_LOST_TEXT),
            SessionEndCause::HostUnreachable => Some(HOST_UNREACHABLE_TEXT),
        }
    }

    fn what_ended_it(self) -> &'static str {
        match self {
            SessionEndCause::PlayerLeft => "you left the lobby",
            SessionEndCause::GaveUp => "you gave up on the host or join under way",
            SessionEndCause::Replaced => "a new host or join from this launcher replaced it",
            SessionEndCause::HostLeft => "the host left",
            SessionEndCause::RemovedByHost => "the host removed you",
            SessionEndCause::Refused(_) => "the host refused the join",
            SessionEndCause::LinkLost => "the link to the host was lost",
            SessionEndCause::HostUnreachable => "the host never answered",
        }
    }
}

impl SessionEndSide {
    fn words(self) -> &'static str {
        match self {
            SessionEndSide::ThisPlayer => "this side",
            SessionEndSide::OtherPlayer => "the other side",
            SessionEndSide::Network => "the network",
        }
    }
}

impl AfterSessionEnd {
    fn words(self) -> &'static str {
        match self {
            AfterSessionEnd::Home => "the launcher went home",
            AfterSessionEnd::StayPut => "the launcher stayed where it was",
            AfterSessionEnd::EndedLobby => "the launcher kept the lobby page, ended",
        }
    }
}

/// The player's own leave goes home. An end from the other side or the network sends a guest on the lobby
/// page home, keeps a host there with the page ended, and leaves anyone elsewhere where they are.
pub fn after_session_end(
    cause: SessionEndCause,
    on_lobby_page: bool,
    viewer_was_guest: bool,
) -> AfterSessionEnd {
    match cause.side() {
        SessionEndSide::ThisPlayer if cause == SessionEndCause::PlayerLeft => AfterSessionEnd::Home,
        SessionEndSide::ThisPlayer => AfterSessionEnd::StayPut,
        SessionEndSide::OtherPlayer | SessionEndSide::Network if !on_lobby_page => {
            AfterSessionEnd::StayPut
        }
        SessionEndSide::OtherPlayer | SessionEndSide::Network if viewer_was_guest => {
            AfterSessionEnd::Home
        }
        SessionEndSide::OtherPlayer | SessionEndSide::Network => AfterSessionEnd::EndedLobby,
    }
}

pub fn session_end_log_line(cause: SessionEndCause, after: AfterSessionEnd) -> String {
    format!(
        "session ended: {}, from {}; {}",
        cause.what_ended_it(),
        cause.side().words(),
        after.words()
    )
}
