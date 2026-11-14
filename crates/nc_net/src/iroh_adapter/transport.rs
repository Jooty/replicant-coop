use std::net::SocketAddr;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, PoisonError};
use std::time::{Duration, Instant};

use iroh::endpoint::presets;
use iroh::{Endpoint, EndpointAddr, RelayMode, Watcher};
use tokio::net::UdpSocket;
use tokio::runtime::{Builder, Runtime};

use crate::iroh_adapter::host_lookup::find_host;
use crate::iroh_adapter::invite_codec::{decode_invite, encode_invite};
use crate::iroh_adapter::key_published::wait_until_key_published;
use crate::iroh_adapter::link::{PeerTable, start_link};
use crate::iroh_adapter::local_answer::{answer_local_lookups, bind_local_answer};
use crate::iroh_adapter::local_lookup::LocalNetworkLookup;
use crate::iroh_adapter::own_address::{relay_label, report_own_address};
use crate::iroh_adapter::path_report::report_paths;
use crate::iroh_adapter::{
    ALPN, BYE_CODE, CLOSE_WAIT_SECONDS, IrohConfig, KEY_PUBLISHED_WAIT, RELAY_CHOICE_WAIT_SECONDS,
    WORKER_THREADS,
};
use crate::progress::{LinkProgress, ProgressSink};
use crate::transport::{
    Channel, Invite, LinkStats, MAX_PAYLOAD_BYTES, NetError, PeerId, Transport, TransportEvent,
};

pub struct IrohTransport {
    runtime: Runtime,
    endpoint: Endpoint,
    config: IrohConfig,
    local: PeerId,
    peers: PeerTable,
    events_tx: Sender<TransportEvent>,
    events_rx: Receiver<TransportEvent>,
    bound_at: Instant,
    progress: Option<ProgressSink>,
    answering_local_lookups: bool,
}

impl IrohTransport {
    pub fn new(config: IrohConfig) -> Result<Self, NetError> {
        let runtime = Builder::new_multi_thread()
            .worker_threads(WORKER_THREADS)
            .thread_name("rc-net-iroh")
            .enable_all()
            .build()
            .map_err(transport_error)?;
        let endpoint = runtime.block_on(bind_endpoint(config))?;
        let bound_at = Instant::now();
        let local = PeerId(*endpoint.id().as_bytes());
        let peers = PeerTable::default();
        let (events_tx, events_rx) = channel();
        runtime.spawn(accept_loop(
            endpoint.clone(),
            Arc::clone(&peers),
            events_tx.clone(),
        ));
        Ok(Self {
            runtime,
            endpoint,
            config,
            local,
            peers,
            events_tx,
            events_rx,
            bound_at,
            progress: None,
            answering_local_lookups: false,
        })
    }

    fn tell(&self, progress: LinkProgress) {
        if let Some(sink) = &self.progress {
            sink.send(progress);
        }
    }

    fn answer_local_lookups(&self, socket: UdpSocket) {
        self.runtime
            .spawn(answer_local_lookups(self.endpoint.clone(), socket));
    }
}

impl Transport for IrohTransport {
    fn local_peer(&self) -> PeerId {
        self.local
    }

    fn host(&mut self) -> Result<Invite, NetError> {
        if !self.answering_local_lookups {
            let port = self.config.local_lookup_port;
            let answer = self.runtime.block_on(bind_local_answer(port));
            self.answering_local_lookups = answer.is_ok();
            self.tell(match answer {
                Ok(socket) => {
                    self.answer_local_lookups(socket);
                    LinkProgress::LocalLookupAnswering { port }
                }
                Err(_) => LinkProgress::LocalLookupOff { port },
            });
        }
        if self.config.use_relays_and_lookup {
            let endpoint = self.endpoint.clone();
            let published = self.runtime.block_on(async move {
                // The record needs the relay's name only: the relay's own answer can come after it.
                let wait = Duration::from_secs(RELAY_CHOICE_WAIT_SECONDS);
                let _ = tokio::time::timeout(wait, wait_for_relay_choice(&endpoint)).await;
                wait_until_key_published(&endpoint).await
            });
            self.tell(match published {
                Some(after) => LinkProgress::KeyPublished { after },
                None => LinkProgress::KeyNotPublished {
                    waited: KEY_PUBLISHED_WAIT,
                },
            });
        }
        encode_invite(self.endpoint.id())
    }

    fn join(&mut self, invite: &Invite) -> Result<(), NetError> {
        let hints = decode_invite(invite)?.hints;
        self.tell(ticket_read(&hints));
        let endpoint = self.endpoint.clone();
        let peers = Arc::clone(&self.peers);
        let events = self.events_tx.clone();
        let progress = self.progress.clone();
        self.runtime.spawn(async move {
            if let Some(sink) = &progress {
                sink.send(LinkProgress::LookingUpHost);
            }
            let found = match find_host(&endpoint, &hints).await {
                Ok(found) => found,
                Err(reason) => {
                    if let Some(sink) = progress {
                        sink.send(LinkProgress::HostNotFound { reason });
                    }
                    return;
                }
            };
            if let Some(sink) = &progress {
                sink.send(found.progress());
            }
            let dialed_at = Instant::now();
            match endpoint.connect(found.addr, ALPN).await {
                Ok(connection) => {
                    if let Some(sink) = progress {
                        sink.send(LinkProgress::HostAnswered {
                            after: dialed_at.elapsed(),
                        });
                        tokio::spawn(report_paths(connection.clone(), sink));
                    }
                    start_link(connection, peers, events);
                }
                Err(_) => {
                    if let Some(sink) = progress {
                        sink.send(LinkProgress::DialFailed);
                    }
                }
            }
        });
        Ok(())
    }

    fn send(&mut self, to: PeerId, channel: Channel, payload: &[u8]) -> Result<(), NetError> {
        if payload.len() > MAX_PAYLOAD_BYTES {
            return Err(NetError::PayloadTooLarge {
                len: payload.len(),
                limit: MAX_PAYLOAD_BYTES,
            });
        }
        let peers = self.peers.lock().unwrap_or_else(PoisonError::into_inner);
        let link = peers.get(&to).ok_or(NetError::NotConnected)?;
        match channel {
            Channel::UnreliableLatest => link
                .connection
                .send_datagram(payload.to_vec().into())
                .map_err(transport_error),
            Channel::ReliableOrdered => link
                .reliable_outbox
                .send(payload.to_vec())
                .map_err(|_| NetError::NotConnected),
        }
    }

    fn poll(&mut self, out: &mut Vec<TransportEvent>) {
        out.extend(self.events_rx.try_iter());
    }

    fn link_stats(&self, peer: PeerId) -> Option<LinkStats> {
        let peers = self.peers.lock().unwrap_or_else(PoisonError::into_inner);
        let connection = &peers.get(&peer)?.connection;
        let paths = connection.paths();
        let selected = paths.iter().find(|path| path.is_selected())?;
        let stats = selected.stats();
        Some(LinkStats {
            rtt: Some(stats.rtt),
            direct: selected.is_ip(),
            lost_packets: stats.lost_packets,
        })
    }

    fn disconnect(&mut self, peer: PeerId) {
        let peers = self.peers.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(link) = peers.get(&peer) {
            link.connection.close(BYE_CODE.into(), b"bye");
        }
    }

    fn report_progress(&mut self, sink: ProgressSink) {
        sink.send_at(self.bound_at, bound(&self.endpoint.bound_sockets()));
        sink.send_at(
            self.bound_at,
            LinkProgress::OwnId(self.endpoint.id().fmt_short().to_string()),
        );
        self.runtime
            .spawn(report_own_address(self.endpoint.clone(), sink.clone()));
        self.progress = Some(sink);
    }
}

impl Drop for IrohTransport {
    fn drop(&mut self) {
        let wait = Duration::from_secs(CLOSE_WAIT_SECONDS);
        let endpoint = self.endpoint.clone();
        let _ = self
            .runtime
            .block_on(async move { tokio::time::timeout(wait, endpoint.close()).await });
    }
}

async fn bind_endpoint(config: IrohConfig) -> Result<Endpoint, NetError> {
    let builder = if config.use_relays_and_lookup {
        Endpoint::builder(presets::N0)
    } else {
        Endpoint::builder(presets::Minimal).relay_mode(RelayMode::Disabled)
    };
    builder
        .address_lookup(LocalNetworkLookup {
            port: config.local_lookup_port,
        })
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .map_err(transport_error)
}

async fn wait_for_relay_choice(endpoint: &Endpoint) {
    let mut addr = endpoint.watch_addr();
    while addr.get().relay_urls().next().is_none() {
        if addr.updated().await.is_err() {
            return;
        }
    }
}

async fn accept_loop(endpoint: Endpoint, peers: PeerTable, events: Sender<TransportEvent>) {
    while let Some(incoming) = endpoint.accept().await {
        let Ok(accepting) = incoming.accept() else {
            continue;
        };
        let peers = Arc::clone(&peers);
        let events = events.clone();
        tokio::spawn(async move {
            if let Ok(connection) = accepting.await {
                start_link(connection, peers, events);
            }
        });
    }
}

fn bound(sockets: &[SocketAddr]) -> LinkProgress {
    let port_of = |ipv6: bool| {
        sockets
            .iter()
            .find(|socket| socket.is_ipv6() == ipv6)
            .map(SocketAddr::port)
    };
    LinkProgress::Bound {
        ipv4_port: port_of(false),
        ipv6_port: port_of(true),
    }
}

fn ticket_read(addr: &EndpointAddr) -> LinkProgress {
    LinkProgress::TicketRead {
        host_id: addr.id.fmt_short().to_string(),
        relay: addr.relay_urls().next().map(relay_label),
        direct_addresses: addr.ip_addrs().count(),
    }
}

fn transport_error(error: impl std::fmt::Display) -> NetError {
    NetError::Transport(error.to_string())
}
