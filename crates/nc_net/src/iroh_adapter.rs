use std::time::Duration;

mod config;
mod host_lookup;
mod invite_codec;
mod key_published;
mod link;
mod local_answer;
mod local_lookup;
mod local_lookup_packet;
mod own_address;
mod path_report;
mod transport;

pub use config::IrohConfig;
pub use invite_codec::invite_reads;
pub use transport::IrohTransport;

const ALPN: &[u8] = b"rc-net/1";
const INVITE_PREFIX: &str = "rcn2";
const ADDRESS_INVITE_PREFIX: &str = "rcn1";
const RELAY_CHOICE_WAIT_SECONDS: u64 = 15;
const CLOSE_WAIT_SECONDS: u64 = 3;
const WORKER_THREADS: usize = 2;
const PROTOCOL_ERROR_CODE: u32 = 1;
const BYE_CODE: u32 = 0;
pub const LOCAL_LOOKUP_PORT: u16 = 47_390;
const LOCAL_LOOKUP_MAGIC: &[u8] = b"ncl1";
const LOCAL_LOOKUP_PACKET_LIMIT: usize = 64;
const LOCAL_LOOKUP_PROVENANCE: &str = "local network";
const LOCAL_LOOKUP_WINDOW: Duration = Duration::from_millis(1200);
const LOCAL_LOOKUP_RESEND_INTERVAL: Duration = Duration::from_millis(200);
const LOCAL_LOOKUP_ANSWER_SETTLE: Duration = Duration::from_millis(60);
const HOST_LOOKUP_GIVE_UP: Duration = Duration::from_secs(10);
const KEY_PUBLISHED_WAIT: Duration = Duration::from_secs(10);
const KEY_PUBLISHED_RECHECK_INTERVAL: Duration = Duration::from_millis(400);
