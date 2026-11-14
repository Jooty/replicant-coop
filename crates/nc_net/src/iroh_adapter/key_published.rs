use std::pin::pin;
use std::time::{Duration, Instant};

use futures_lite::StreamExt;
use iroh::Endpoint;

use crate::iroh_adapter::{
    KEY_PUBLISHED_RECHECK_INTERVAL, KEY_PUBLISHED_WAIT, LOCAL_LOOKUP_PROVENANCE,
};

pub(super) async fn wait_until_key_published(endpoint: &Endpoint) -> Option<Duration> {
    // The key is looked up the way a joiner looks it up, until the record names the current relay.
    let started = Instant::now();
    let give_up_at = started + KEY_PUBLISHED_WAIT;
    while Instant::now() < give_up_at {
        let left = give_up_at.saturating_duration_since(Instant::now());
        if let Ok(true) = tokio::time::timeout(left, published_record_shows_relay(endpoint)).await {
            return Some(started.elapsed());
        }
        tokio::time::sleep(KEY_PUBLISHED_RECHECK_INTERVAL).await;
    }
    None
}

async fn published_record_shows_relay(endpoint: &Endpoint) -> bool {
    let Some(relay) = endpoint.addr().relay_urls().next().cloned() else {
        return false;
    };
    let Ok(services) = endpoint.address_lookup() else {
        return false;
    };
    let mut answers = pin!(services.resolve(endpoint.id()));
    while let Some(answer) = answers.next().await {
        // This computer's own local answer proves nothing about the published record.
        if let Ok(Ok(item)) = answer
            && item.provenance() != LOCAL_LOOKUP_PROVENANCE
            && item.relay_urls().any(|url| *url == relay)
        {
            return true;
        }
    }
    false
}
