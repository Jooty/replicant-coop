use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::time::Instant;

use iroh::{Endpoint, RelayUrl, Watcher};

use crate::progress::{AddressKind, LinkProgress, ProgressSink};

#[derive(Default)]
struct OwnAddressSeen {
    chosen: Vec<(RelayUrl, Instant)>,
    answered: Vec<RelayUrl>,
    direct: BTreeSet<SocketAddr>,
}

impl OwnAddressSeen {
    fn note_relays(&mut self, relays: &[iroh::endpoint::RelayStatus], sink: &ProgressSink) {
        for status in relays {
            let url = status.url();
            let chosen_at = match self.chosen.iter().find(|(chosen, _)| chosen == url) {
                Some((_, at)) => *at,
                None => {
                    let now = Instant::now();
                    self.chosen.push((url.clone(), now));
                    sink.send_at(
                        now,
                        LinkProgress::RelayChosen {
                            relay: relay_label(url),
                        },
                    );
                    now
                }
            };
            if status.is_connected() && !self.answered.contains(url) {
                self.answered.push(url.clone());
                sink.send(LinkProgress::RelayAnswered {
                    relay: relay_label(url),
                    after: chosen_at.elapsed(),
                });
            }
        }
    }

    fn note_direct(&mut self, addr: &iroh::EndpointAddr, sink: &ProgressSink) {
        for ip in addr.ip_addrs() {
            if self.direct.insert(*ip) {
                sink.send(LinkProgress::DirectAddressFound {
                    kind: AddressKind::of(ip.ip()),
                    count: self.direct.len(),
                });
            }
        }
    }
}

pub(super) async fn report_own_address(endpoint: Endpoint, sink: ProgressSink) {
    // Runs until the endpoint closes.
    let closed = endpoint.closed();
    closed.run_until(watch_own_address(endpoint, sink)).await;
}

pub(super) fn relay_label(url: &RelayUrl) -> String {
    // The first part of the server's name says where the relay stands.
    let host = url.host_str().unwrap_or_default();
    host.split('.').next().unwrap_or(host).to_owned()
}

async fn watch_own_address(endpoint: Endpoint, sink: ProgressSink) {
    let mut relays = endpoint.home_relay_status();
    let mut addr = endpoint.watch_addr();
    let mut seen = OwnAddressSeen::default();
    loop {
        seen.note_relays(&relays.get(), &sink);
        seen.note_direct(&addr.get(), &sink);
        tokio::select! {
            changed = relays.updated() => if changed.is_err() { return },
            changed = addr.updated() => if changed.is_err() { return },
        }
    }
}
