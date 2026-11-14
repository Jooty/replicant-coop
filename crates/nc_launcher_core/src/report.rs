mod connection_report;

pub const HOST_KEY_NOT_FOUND_TEXT: &str = "Could not find the host by its ticket. Check that the ticket is current and the host's launcher is still hosting, then join again.";
pub const HOST_UNREACHABLE_TEXT: &str = "Could not reach the host. Check that the ticket is current and the host's launcher is running. If you are both behind strict routers, wait a minute and try again.";

pub use connection_report::{
    ConnectionReport, PathKind, Quality, RttStats, Verdict, judge, judge_rejection_text,
};
