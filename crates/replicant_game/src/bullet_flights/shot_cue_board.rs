use replicant_proto::{BULLET_TRANSFORM_FLOATS, ShotCue, ShotCueKind};

pub const NO_COPY_TO_FOLLOW: &str = "with no copy to follow";
pub const NO_COPY_TO_END: &str = "with no copy to end";

/// What the copy does with a shot cue as it arrives.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CueOnCopy {
    /// Make the following effect or lasting sound on the live copy of `instance`.
    Follow {
        instance: u16,
    },
    /// End the copy's following effects or its lasting sound.
    End {
        instance: u16,
    },
    /// Make the one-shot now at this transform.
    Place {
        transform: [f32; BULLET_TRANSFORM_FLOATS],
    },
    /// Held until the tick's flights are applied, then placed by `take_due`.
    Wait,
    Drop {
        why: &'static str,
    },
}

/// The copy's side: puts each effect and sound a partner's shot made on the copy of that shot. A
/// one-shot lands where the copy is when the tick's flights are applied, or where it ended when
/// its end came in the same tick; with no copy, where the owner made it.
#[derive(Debug, Clone, Default)]
pub struct ShotCueBoard {
    waiting: Vec<ShotCue>,
    ended: Vec<(u16, [f32; BULLET_TRANSFORM_FLOATS])>,
}

impl ShotCueBoard {
    /// `live` says whether a copy of the cue's shot is drawn now.
    pub fn note(&mut self, cue: ShotCue, live: bool) -> CueOnCopy {
        let known = cue
            .instance
            .filter(|instance| live || self.ended_transform(*instance).is_some());
        match cue.kind {
            ShotCueKind::FollowingEffect | ShotCueKind::LastingSound => {
                match cue.instance.filter(|_| live) {
                    Some(instance) => CueOnCopy::Follow { instance },
                    None => CueOnCopy::Drop {
                        why: NO_COPY_TO_FOLLOW,
                    },
                }
            }
            ShotCueKind::EndLastingSound | ShotCueKind::EndFollowingEffects => {
                match cue.instance.filter(|_| live) {
                    Some(instance) => CueOnCopy::End { instance },
                    None => CueOnCopy::Drop {
                        why: NO_COPY_TO_END,
                    },
                }
            }
            ShotCueKind::OneShotEffect | ShotCueKind::OneShotSound => {
                if known.is_some() {
                    self.waiting.push(cue);
                    CueOnCopy::Wait
                } else {
                    CueOnCopy::Place {
                        transform: cue.transform,
                    }
                }
            }
        }
    }

    pub fn note_stop(&mut self, instance: u16, last_transform: [f32; BULLET_TRANSFORM_FLOATS]) {
        self.ended.retain(|(known, _)| *known != instance);
        self.ended.push((instance, last_transform));
    }

    /// The waiting one-shots with where each lands; clears the tick's ends.
    pub fn take_due(
        &mut self,
        live_transform: impl Fn(u16) -> Option<[f32; BULLET_TRANSFORM_FLOATS]>,
    ) -> Vec<(ShotCue, [f32; BULLET_TRANSFORM_FLOATS])> {
        let waiting = core::mem::take(&mut self.waiting);
        let due = waiting
            .into_iter()
            .map(|cue| {
                let copy = cue.instance.and_then(|instance| {
                    self.ended_transform(instance)
                        .or_else(|| live_transform(instance))
                });
                let transform = one_shot_transform(&cue, copy.as_ref());
                (cue, transform)
            })
            .collect();
        self.ended.clear();
        due
    }

    fn ended_transform(&self, instance: u16) -> Option<[f32; BULLET_TRANSFORM_FLOATS]> {
        self.ended
            .iter()
            .find(|(known, _)| *known == instance)
            .map(|(_, transform)| *transform)
    }
}

/// The owner's transform for the cue, moved to the copy's position plus the owner's offset from
/// its shot; the owner's own place when there is no copy.
pub fn one_shot_transform(
    cue: &ShotCue,
    copy: Option<&[f32; BULLET_TRANSFORM_FLOATS]>,
) -> [f32; BULLET_TRANSFORM_FLOATS] {
    let Some(copy) = copy else {
        return cue.transform;
    };
    let mut transform = cue.transform;
    for (axis, index) in [3, 7, 11].into_iter().enumerate() {
        transform[index] = copy[index] + cue.offset[axis];
    }
    transform
}
