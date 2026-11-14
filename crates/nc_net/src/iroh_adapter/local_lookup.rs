use std::collections::BTreeSet;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Instant;

use futures_lite::stream::{self, Boxed};
use iroh::EndpointId;
use iroh::address_lookup::{AddressLookup, Error, Item};
use iroh::endpoint_info::{EndpointData, EndpointInfo};
use tokio::net::UdpSocket;

use crate::iroh_adapter::local_lookup_packet::LocalLookupPacket;
use crate::iroh_adapter::{
    LOCAL_LOOKUP_ANSWER_SETTLE, LOCAL_LOOKUP_PACKET_LIMIT, LOCAL_LOOKUP_PROVENANCE,
    LOCAL_LOOKUP_RESEND_INTERVAL, LOCAL_LOOKUP_WINDOW,
};

/// Finds an endpoint by its key on this computer and on the local network: a question goes to this
/// computer and to the network's broadcast address, and the endpoint holding the key answers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LocalNetworkLookup {
    pub(super) port: u16,
}

#[derive(Debug, thiserror::Error)]
#[error("no answer on the local network")]
struct NoLocalAnswer;

impl AddressLookup for LocalNetworkLookup {
    fn resolve(&self, endpoint_id: EndpointId) -> Option<Boxed<Result<Item, Error>>> {
        let port = self.port;
        let found = async move {
            let addrs = ask_local_network(endpoint_id, port).await;
            if addrs.is_empty() {
                return Err(Error::from_err(LOCAL_LOOKUP_PROVENANCE, NoLocalAnswer));
            }
            let info = EndpointInfo::from_parts(endpoint_id, EndpointData::from(addrs));
            Ok(Item::new(info, LOCAL_LOOKUP_PROVENANCE, None))
        };
        Some(Box::pin(stream::once_future(found)))
    }
}

async fn ask_local_network(endpoint_id: EndpointId, port: u16) -> BTreeSet<SocketAddr> {
    let mut found = BTreeSet::new();
    let Ok(socket) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await else {
        return found;
    };
    let broadcast_allowed = socket.set_broadcast(true).is_ok();
    let query = LocalLookupPacket::query(endpoint_id).to_bytes();
    let this_computer = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
    let whole_network = SocketAddrV4::new(Ipv4Addr::BROADCAST, port);
    let mut give_up_at = Instant::now() + LOCAL_LOOKUP_WINDOW;
    let mut next_query_at = Instant::now();
    let mut buffer = [0_u8; LOCAL_LOOKUP_PACKET_LIMIT];
    loop {
        let now = Instant::now();
        if now >= give_up_at {
            return found;
        }
        if found.is_empty() && now >= next_query_at {
            // A network with no broadcast route refuses the second send; this computer is still asked.
            let _ = socket.send_to(&query, this_computer).await;
            if broadcast_allowed {
                let _ = socket.send_to(&query, whole_network).await;
            }
            next_query_at = now + LOCAL_LOOKUP_RESEND_INTERVAL;
        }
        let wake_at = if found.is_empty() {
            next_query_at.min(give_up_at)
        } else {
            give_up_at
        };
        let wait = wake_at.saturating_duration_since(Instant::now());
        // Windows reports a refused earlier send as a failed receive: it is waited past, not resent at once.
        if let Ok(Ok((len, from))) = tokio::time::timeout(wait, socket.recv_from(&mut buffer)).await
            && let Some(LocalLookupPacket::Answer { key, ipv4_port }) =
                LocalLookupPacket::read(&buffer[..len])
            && key == *endpoint_id.as_bytes()
        {
            if found.is_empty() {
                give_up_at = Instant::now() + LOCAL_LOOKUP_ANSWER_SETTLE;
            }
            found.insert(SocketAddr::new(from.ip(), ipv4_port));
        }
    }
}
