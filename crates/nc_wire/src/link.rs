use serde::{Deserialize, Serialize};

use crate::{BuildGate, SessionRules};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub version: u16,
    pub build: BuildGate,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Welcome {
    pub version: u16,
    pub slot: u8,
    pub host_name: String,
    pub rules: SessionRules,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectReason {
    VersionMismatch,
    BuildMismatch,
    SessionFull,
    Malformed,
    RemovedByHost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reject {
    pub reason: RejectReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ping {
    pub nonce: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pong {
    pub nonce: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub slot: u8,
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberPing {
    pub slot: u8,
    pub rtt_ms: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkPayload {
    Hello(Hello),
    Welcome(Welcome),
    Reject(Reject),
    Ping(Ping),
    Pong(Pong),
    Bye,
    Member(Member),
    MemberLeft { slot: u8 },
    Rules(SessionRules),
    MemberPings(Vec<MemberPing>),
}
