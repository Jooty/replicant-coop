use std::collections::HashMap;

use replicant_proto::ShotCueKind;

use crate::enemy_bullets::{MAX_SHOT_CUE_REPEATS_REMEMBERED, SHOT_CUE_REPEAT_GAP_MS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CueAdmission {
    Send,
    RepeatTooSoon,
}

/// The owner's side: which effects and sounds its shots make go to the copy. The first of each
/// kind and id a shot makes always goes, so no burst of shots loses an impact; the same one
/// again on the same shot goes at most once per `SHOT_CUE_REPEAT_GAP_MS`.
#[derive(Debug, Default)]
pub struct ShotCueGate {
    last_sent_ms: HashMap<(u64, ShotCueKind, u32), u64>,
}

impl ShotCueGate {
    pub fn admit(&mut self, shot: u64, kind: ShotCueKind, id: u32, now_ms: u64) -> CueAdmission {
        if !kind.is_effect() && !kind.is_sound() {
            return CueAdmission::Send;
        }
        if self.last_sent_ms.len() >= MAX_SHOT_CUE_REPEATS_REMEMBERED {
            self.last_sent_ms
                .retain(|_, sent_ms| now_ms.saturating_sub(*sent_ms) < SHOT_CUE_REPEAT_GAP_MS);
        }
        let key = (shot, kind, id);
        if let Some(sent_ms) = self.last_sent_ms.get(&key)
            && now_ms.saturating_sub(*sent_ms) < SHOT_CUE_REPEAT_GAP_MS
        {
            return CueAdmission::RepeatTooSoon;
        }
        self.last_sent_ms.insert(key, now_ms);
        CueAdmission::Send
    }
}
