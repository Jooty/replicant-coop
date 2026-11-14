use std::collections::BTreeMap;

/// The host's count of enemy bullet flights it did not send because they failed the wire
/// check, by the check's reason, for one summary window.
#[derive(Debug, Clone, Default)]
pub struct FlightRefusals {
    by_reason: BTreeMap<String, u32>,
}

impl FlightRefusals {
    pub fn note(&mut self, reason: String) {
        *self.by_reason.entry(reason).or_default() += 1;
    }

    /// The summary of the window that ends now, if any flight was refused in it.
    pub fn take_summary(&mut self) -> Option<String> {
        let by_reason = core::mem::take(&mut self.by_reason);
        if by_reason.is_empty() {
            return None;
        }
        let total: u32 = by_reason.values().sum();
        let parts: Vec<String> = by_reason
            .iter()
            .map(|(reason, count)| format!("{reason} x{count}"))
            .collect();
        Some(format!(
            "enemy bullets: last 5 s {total} flights failed the wire check and were not sent: {}",
            parts.join(", ")
        ))
    }
}
