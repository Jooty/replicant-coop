use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn new_campaign_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let mut high = RandomState::new().build_hasher();
    high.write_u128(nanos);
    let mut low = RandomState::new().build_hasher();
    low.write_u128(nanos);
    campaign_id_from(high.finish(), low.finish())
}

pub fn campaign_id_from(high: u64, low: u64) -> String {
    format!("{high:016x}{low:016x}")
}
