use std::collections::{BTreeMap, BTreeSet};

use replicant_proto::ShotCueKind;

/// One side's count of shot cues over a summary window, for one log line.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShotCueTally {
    shots: BTreeSet<u16>,
    trails: u32,
    impacts: u32,
    sounds: u32,
    ends: u32,
    resizes: u32,
    largest_scale: f32,
    dropped: BTreeMap<&'static str, u32>,
}

impl ShotCueTally {
    pub fn note_cue(&mut self, instance: Option<u16>, kind: ShotCueKind) {
        if let Some(instance) = instance {
            self.shots.insert(instance);
        }
        match kind {
            ShotCueKind::FollowingEffect => self.trails += 1,
            ShotCueKind::OneShotEffect => self.impacts += 1,
            ShotCueKind::OneShotSound | ShotCueKind::LastingSound => self.sounds += 1,
            ShotCueKind::EndLastingSound | ShotCueKind::EndFollowingEffects => self.ends += 1,
        }
    }

    /// A shot's look carried new sizes for its effects, the largest axis now `scale`.
    pub fn note_resize(&mut self, instance: u16, scale: f32) {
        self.shots.insert(instance);
        self.resizes += 1;
        self.largest_scale = self.largest_scale.max(scale);
    }

    pub fn note_dropped(&mut self, why: &'static str) {
        *self.dropped.entry(why).or_default() += 1;
    }

    /// The line for the window, then a fresh count; `None` when no cue came and no shot flies.
    /// `done` says what this side did with the cues: "sent" or "made".
    pub fn take_line(&mut self, done: &str, shots_in_the_air: usize) -> Option<String> {
        let tally = core::mem::take(self);
        let cues = tally.trails + tally.impacts + tally.sounds + tally.ends;
        let dropped: u32 = tally.dropped.values().sum();
        if cues + dropped + tally.resizes == 0 && shots_in_the_air == 0 {
            return None;
        }
        let why = if dropped == 0 {
            String::new()
        } else {
            let reasons: Vec<String> = tally
                .dropped
                .iter()
                .map(|(why, count)| format!("{count} {why}"))
                .collect();
            format!(" ({})", reasons.join(", "))
        };
        let resizes = if tally.resizes == 0 {
            "0 resizes".to_owned()
        } else {
            format!(
                "{} resizes up to scale {:.1}",
                tally.resizes, tally.largest_scale
            )
        };
        Some(format!(
            "shot cues: last 1 s {done} {cues} for {} shots ({} trails, {} impacts, {} sounds, {} ends, {resizes}), {shots_in_the_air} shots in the air, dropped {dropped}{why}",
            tally.shots.len(),
            tally.trails,
            tally.impacts,
            tally.sounds,
            tally.ends
        ))
    }
}
