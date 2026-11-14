use std::net::{Ipv4Addr, SocketAddr};

use iroh::Endpoint;
use tokio::net::UdpSocket;

use crate::iroh_adapter::LOCAL_LOOKUP_PACKET_LIMIT;
use crate::iroh_adapter::local_lookup_packet::LocalLookupPacket;

pub(super) async fn bind_local_answer(port: u16) -> std::io::Result<UdpSocket> {
    UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port)).await
}

pub(super) async fn answer_local_lookups(endpoint: Endpoint, socket: UdpSocket) {
    // Answers until the endpoint closes; the socket closes with this task.
    tokio::select! {
        () = endpoint.closed() => {}
        () = answer_until_error(&endpoint, &socket) => {}
    }
}

async fn answer_until_error(endpoint: &Endpoint, socket: &UdpSocket) {
    let own_key = *endpoint.id().as_bytes();
    let mut buffer = [0_u8; LOCAL_LOOKUP_PACKET_LIMIT];
    loop {
        let Ok((len, from)) = socket.recv_from(&mut buffer).await else {
            return;
        };
        let Some(LocalLookupPacket::Query { key }) = LocalLookupPacket::read(&buffer[..len]) else {
            continue;
        };
        let Some(ipv4_port) = ipv4_port(&endpoint.bound_sockets()) else {
            continue;
        };
        if key == own_key {
            let answer = LocalLookupPacket::Answer { key, ipv4_port };
            // A lost answer is asked for again by the next query.
            let _ = socket.send_to(&answer.to_bytes(), from).await;
        }
    }
}

fn ipv4_port(sockets: &[SocketAddr]) -> Option<u16> {
    sockets
        .iter()
        .find(|socket| socket.is_ipv4())
        .map(SocketAddr::port)
}
