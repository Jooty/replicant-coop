use replicant_proto::GameReadiness;

use crate::net::LOBBY_MODE_CONTINUE;

pub const GAME_QUIT_GRACE_SECONDS: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowNotice {
    YourGameClosed,
    HostGameClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LastLaunch {
    pub guest: bool,
    pub mode: u8,
    pub slot: Option<u8>,
}

#[derive(Debug, Clone, Copy)]
pub struct HostClosedInputs {
    pub guest: bool,
    pub session_up: bool,
    pub lobby_started: bool,
    pub waiting_for_host_game: bool,
    pub own_game_closed: bool,
    pub partner_seen_linked: bool,
    pub partner: GameReadiness,
    pub notice_shown: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitStep {
    Waiting,
    EndProcesses,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuitFallback {
    asked_at: f64,
    ended: bool,
}

impl QuitFallback {
    pub fn new(now_seconds: f64) -> Self {
        Self {
            asked_at: now_seconds,
            ended: false,
        }
    }

    pub fn step(&mut self, now_seconds: f64) -> QuitStep {
        if self.ended {
            return QuitStep::Done;
        }
        if now_seconds - self.asked_at >= GAME_QUIT_GRACE_SECONDS {
            self.ended = true;
            return QuitStep::EndProcesses;
        }
        QuitStep::Waiting
    }
}

pub fn own_close_notice(session_up: bool, host_close_in_progress: bool) -> Option<FollowNotice> {
    if host_close_in_progress {
        return Some(FollowNotice::HostGameClosed);
    }
    session_up.then_some(FollowNotice::YourGameClosed)
}

pub fn host_game_closed(inputs: HostClosedInputs) -> bool {
    inputs.guest
        && inputs.session_up
        && inputs.lobby_started
        && !inputs.waiting_for_host_game
        && !inputs.own_game_closed
        && !inputs.notice_shown
        && inputs.partner_seen_linked
        && inputs.partner == GameReadiness::NotLinked
}

pub fn relaunch_choice(last: LastLaunch, slot_exists: impl Fn(u8) -> bool) -> (u8, Option<u8>) {
    match last.slot {
        Some(slot) if slot_exists(slot) => (LOBBY_MODE_CONTINUE, Some(slot)),
        _ => (last.mode, None),
    }
}
