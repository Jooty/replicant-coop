use std::time::Duration;

use replicant_proto::RejectReason;

use crate::report::HOST_UNREACHABLE_TEXT;

const GOOD_RTT_MILLIS: f64 = 60.0;
const FAIR_RTT_MILLIS: f64 = 120.0;
const GOOD_LOSS_PERCENT: f32 = 2.0;
const FAIR_LOSS_PERCENT: f32 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathKind {
    Direct,
    Relay,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    Good,
    Fair,
    Poor,
    Unusable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RttStats {
    pub min: Duration,
    pub avg: Duration,
    pub max: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub quality: Quality,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConnectionReport {
    pub reachable: bool,
    pub path: PathKind,
    pub rtt: Option<RttStats>,
    pub loss_percent: f32,
    pub rejected: Option<RejectReason>,
    pub host_name: Option<String>,
    pub verdict: String,
}

impl RttStats {
    pub fn from_samples(samples: &[Duration]) -> Option<Self> {
        let min = *samples.iter().min()?;
        let max = *samples.iter().max()?;
        let total: Duration = samples.iter().sum();
        let count = u32::try_from(samples.len()).ok()?;
        Some(Self {
            min,
            avg: total / count,
            max,
        })
    }
}

pub fn judge(
    reachable: bool,
    rejected: Option<RejectReason>,
    path: PathKind,
    rtt: Option<RttStats>,
    loss_percent: f32,
) -> Verdict {
    if let Some(reason) = rejected {
        return Verdict {
            quality: Quality::Unusable,
            summary: judge_rejection_text(reason).to_owned(),
        };
    }
    let Some(rtt) = rtt.filter(|_| reachable) else {
        return Verdict {
            quality: Quality::Unusable,
            summary: HOST_UNREACHABLE_TEXT.to_owned(),
        };
    };
    let avg_millis = rtt.avg.as_secs_f64() * 1000.0;
    let quality = if avg_millis < GOOD_RTT_MILLIS && loss_percent < GOOD_LOSS_PERCENT {
        Quality::Good
    } else if avg_millis < FAIR_RTT_MILLIS && loss_percent < FAIR_LOSS_PERCENT {
        Quality::Fair
    } else {
        Quality::Poor
    };
    let opening = match quality {
        Quality::Good => "The connection is good.",
        Quality::Fair => "The connection works but may stutter.",
        _ => "The connection is poor and play will feel laggy.",
    };
    let route = match path {
        PathKind::Direct => "It is a direct link.",
        PathKind::Relay => "It goes through a relay server, which adds delay.",
        PathKind::Unknown => "The route is not known yet.",
    };
    Verdict {
        quality,
        summary: format!(
            "{opening} {route} Average delay {avg_millis:.0} ms, {loss_percent:.0}% of test packets lost."
        ),
    }
}

pub fn judge_rejection_text(reason: RejectReason) -> &'static str {
    match reason {
        RejectReason::BuildMismatch => "The host runs a different game version than this one.",
        RejectReason::VersionMismatch => {
            "The host runs a different launcher version. Both of you need the same release."
        }
        RejectReason::SessionFull => {
            "The session is full: the host already plays with another player."
        }
        RejectReason::Malformed => "The host could not understand the request.",
        RejectReason::RemovedByHost => "The host removed you from the lobby.",
    }
}
