use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NameCount {
    pub seen: u32,
    pub passed: u32,
}

/// Effects counted by name over one summary window: how often each was seen and how often it
/// passed (was sent, or was shown).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectNameTally {
    counts: BTreeMap<String, NameCount>,
}

impl EffectNameTally {
    pub fn note(&mut self, name: &str, passed: bool) {
        let count = self.counts.entry(name.to_owned()).or_default();
        count.seen = count.seen.saturating_add(1);
        if passed {
            count.passed = count.passed.saturating_add(1);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// Every name with its counts, most seen first, and the window starts again.
    pub fn take(&mut self) -> Vec<(String, NameCount)> {
        let mut names: Vec<(String, NameCount)> =
            core::mem::take(&mut self.counts).into_iter().collect();
        names.sort_by(|a, b| b.1.seen.cmp(&a.1.seen).then_with(|| a.0.cmp(&b.0)));
        names
    }
}

/// `name xN` items joined by commas, at most `limit` of them, then how many names were left out.
pub fn name_counts_text(
    names: &[(String, NameCount)],
    limit: usize,
    item: impl Fn(&str, NameCount) -> String,
) -> String {
    let mut parts: Vec<String> = names
        .iter()
        .take(limit)
        .map(|(name, count)| item(name, *count))
        .collect();
    if names.len() > limit {
        parts.push(format!("and {} more names", names.len() - limit));
    }
    parts.join(", ")
}

/// The receiving game's line for the world effects the partner sent, by name.
pub fn world_effects_shown_line(tally: &mut EffectNameTally, limit: usize) -> Option<String> {
    if tally.is_empty() {
        return None;
    }
    let names = tally.take();
    let text = name_counts_text(&names, limit, |name, count| {
        let missing = count.seen - count.passed;
        if missing == 0 {
            format!("{name} x{} (all shown)", count.seen)
        } else {
            format!(
                "{name} x{} ({} shown, {missing} not created)",
                count.seen, count.passed
            )
        }
    });
    Some(format!(
        "effects: last 5 s world effects from the partner by name: {text}"
    ))
}
