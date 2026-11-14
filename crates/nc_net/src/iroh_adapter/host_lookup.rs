use std::pin::pin;
use std::time::{Duration, Instant};

use futures_lite::StreamExt;
use iroh::address_lookup::Item;
use iroh::{Endpoint, EndpointAddr};

use crate::iroh_adapter::own_address::relay_label;
use crate::iroh_adapter::{HOST_LOOKUP_GIVE_UP, LOCAL_LOOKUP_PROVENANCE, LOCAL_LOOKUP_WINDOW};
use crate::progress::{LinkProgress, LookupMeans};

/// Where the host was found, and the addresses to dial it on.
pub(super) struct FoundHost {
    pub(super) addr: EndpointAddr,
    pub(super) by: LookupMeans,
    pub(super) after: Duration,
}

impl FoundHost {
    pub(super) fn progress(&self) -> LinkProgress {
        LinkProgress::HostFound {
            by: self.by,
            after: self.after,
            relay: self.addr.relay_urls().next().map(relay_label),
            direct_addresses: self.addr.ip_addrs().count(),
        }
    }
}

pub(super) async fn find_host(
    endpoint: &Endpoint,
    hints: &EndpointAddr,
) -> Result<FoundHost, String> {
    // A local network answer is taken first, a published record when the local network stays quiet,
    // and an older ticket's own addresses when nothing answers at all.
    let started = Instant::now();
    let services = endpoint
        .address_lookup()
        .map_err(|_| "the link closed before the lookup".to_owned())?;
    let mut answers = pin!(services.resolve(hints.id));
    let mut published: Option<Item> = None;
    let mut failed: Vec<String> = Vec::new();
    loop {
        let give_up_at = if published.is_some() {
            started + LOCAL_LOOKUP_WINDOW
        } else {
            started + HOST_LOOKUP_GIVE_UP
        };
        let wait = give_up_at.saturating_duration_since(Instant::now());
        match tokio::time::timeout(wait, answers.next()).await {
            Ok(Some(Ok(Ok(item)))) if item.provenance() == LOCAL_LOOKUP_PROVENANCE => {
                return Ok(found(item, LookupMeans::LocalNetwork, hints, started));
            }
            Ok(Some(Ok(Ok(item)))) => {
                published.get_or_insert(item);
            }
            Ok(Some(Ok(Err(error)))) => failed.push(service_name(&error.to_string())),
            Ok(Some(Err(_)) | None) | Err(_) => break,
        }
    }
    if let Some(item) = published {
        return Ok(found(item, LookupMeans::PublishedRecord, hints, started));
    }
    if !hints.is_empty() {
        return Ok(FoundHost {
            addr: hints.clone(),
            by: LookupMeans::TicketAddresses,
            after: started.elapsed(),
        });
    }
    Err(not_found_reason(&failed, started.elapsed()))
}

fn found(item: Item, by: LookupMeans, hints: &EndpointAddr, started: Instant) -> FoundHost {
    let addr = item
        .into_endpoint_addr()
        .with_addrs(hints.addrs.iter().cloned());
    FoundHost {
        addr,
        by,
        after: started.elapsed(),
    }
}

fn service_name(failure: &str) -> String {
    // The library names a failed service as "Service 'name' failed"; only the name is kept.
    failure.split('\'').nth(1).unwrap_or(failure).to_owned()
}

fn not_found_reason(failed: &[String], waited: Duration) -> String {
    if failed.is_empty() {
        format!("no lookup answered within {} s", waited.as_secs())
    } else {
        format!("no lookup knew the key ({})", failed.join(", "))
    }
}
