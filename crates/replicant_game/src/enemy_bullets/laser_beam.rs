use std::collections::BTreeSet;

use replicant_proto::{BULLET_TRANSFORM_FLOATS, EnemyId};

use crate::enemies::ActorHandle;
use crate::enemy_bullets::{EnemyBullet, MAX_LASER_BEAM_KINDS_NAMED, script_bullet_is_laser};
use crate::memory::GameMemory;
use crate::offsets::{
    ACTOR_TRANSFORM_BLOCK_BYTES, ACTOR_TRANSFORM_BLOCK_OFFSET,
    SCRIPT_BULLET_EFFECT_OBJECT_POINTER_OFFSET, SCRIPT_BULLET_SENIOR_HANDLE_OFFSET,
    SCRIPT_BULLET_TO_SENIOR_EFFECT_LENGTH_OFFSET, SCRIPT_BULLET_TO_SENIOR_EFFECT_VTABLE_RVA,
    SCRIPT_BULLET_VTABLE_RVA,
};

const FLOAT_BYTES: usize = 4;
const TRANSFORM_ROW_FLOATS: usize = 4;
const TRANSLATION_COLUMN: usize = 3;
const AXES: usize = 3;

/// What the far end of a laser beam is: the bullet the beam's effect is stretched to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BeamEnd {
    SpawnedAsLaser,
    PlainScriptBullet,
    OtherActor,
    Gone,
}

/// A script bullet whose effect the game stretches each frame from the bullet to another bullet
/// (its senior): one laser beam, drawn by that one effect.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaserBeam {
    pub far_end: BeamEnd,
    /// World units from the bullet to its far end.
    pub length: f32,
    /// The length one unit of the effect covers; the game scales the effect by `length` over it.
    pub effect_length: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct BeamKind {
    group: u16,
    ordinal: u16,
    effect_id: i32,
    far_end: BeamEnd,
}

/// The host's record of laser beams in one summary window: which beams were in the air, which
/// of them were sent and how often, and which kinds it has already named in the log.
#[derive(Debug, Clone, Default)]
pub struct LaserBeamTally {
    in_the_air: BTreeSet<u16>,
    sent: BTreeSet<u16>,
    messages: u32,
    most_at_once: u32,
    longest: f32,
    longest_axis: f32,
    named: BTreeSet<BeamKind>,
}

impl LaserBeamTally {
    /// Counts one scan's beams; returns a line for each beam kind seen for the first time.
    pub fn add_scan(&mut self, bullets: &[EnemyBullet]) -> Vec<String> {
        let mut first_lines = Vec::new();
        let mut at_once = 0_u32;
        for seen in bullets {
            let Some(beam) = seen.beam else {
                continue;
            };
            at_once += 1;
            self.in_the_air.insert(seen.bullet.slot);
            let axis = longest_axis(&seen.bullet.transform);
            self.longest = self.longest.max(beam.length);
            self.longest_axis = self.longest_axis.max(axis);
            let kind = BeamKind {
                group: seen.enemy.group,
                ordinal: seen.enemy.ordinal,
                effect_id: seen.bullet.look.effect_ids[0],
                far_end: beam.far_end,
            };
            if self.named.len() < MAX_LASER_BEAM_KINDS_NAMED && self.named.insert(kind) {
                first_lines.push(first_beam_line(seen.enemy, kind.effect_id, beam, axis));
            }
        }
        self.most_at_once = self.most_at_once.max(at_once);
        first_lines
    }

    /// Counts one flight message the host sent for the bullet in `slot`, if it is a beam.
    pub fn note_sent(&mut self, slot: u16) {
        if self.in_the_air.contains(&slot) {
            self.messages += 1;
            self.sent.insert(slot);
        }
    }

    /// The summary of the window that ends now, if any beam was in the air in it.
    pub fn take_summary(&mut self) -> Option<String> {
        if self.in_the_air.is_empty() {
            return None;
        }
        let line = format!(
            "enemy bullets: last 5 s laser beams: {} in the air (most at once {}), {} of them sent in {} flight messages, longest {:.1} units (longest axis {:.1})",
            self.in_the_air.len(),
            self.most_at_once,
            self.sent.len(),
            self.messages,
            self.longest,
            self.longest_axis
        );
        let named = core::mem::take(&mut self.named);
        *self = Self {
            named,
            ..Self::default()
        };
        Some(line)
    }
}

/// The laser beam a script bullet draws, if its effect object is the kind stretched to a senior
/// bullet.
pub fn read_laser_beam<M: GameMemory>(memory: &M, actor: u64) -> Option<LaserBeam> {
    let object = memory
        .read_u64(actor + SCRIPT_BULLET_EFFECT_OBJECT_POINTER_OFFSET)
        .ok()
        .filter(|object| *object != 0)?;
    let base = memory.module_base();
    if memory.read_u64(object).ok()? != base + u64::from(SCRIPT_BULLET_TO_SENIOR_EFFECT_VTABLE_RVA)
    {
        return None;
    }
    let effect_length = memory
        .read_f32(object + SCRIPT_BULLET_TO_SENIOR_EFFECT_LENGTH_OFFSET)
        .unwrap_or(0.0);
    let senior = ActorHandle::read(memory, actor + SCRIPT_BULLET_SENIOR_HANDLE_OFFSET)
        .and_then(|handle| handle.resolve(memory));
    let (far_end, length) = match senior {
        None => (BeamEnd::Gone, 0.0),
        Some(senior) => {
            let far_end = if memory.read_u64(senior).ok()
                != Some(base + u64::from(SCRIPT_BULLET_VTABLE_RVA))
            {
                BeamEnd::OtherActor
            } else if script_bullet_is_laser(memory, senior) {
                BeamEnd::SpawnedAsLaser
            } else {
                BeamEnd::PlainScriptBullet
            };
            let length = match (position_of(memory, actor), position_of(memory, senior)) {
                (Some(near), Some(far)) => distance(near, far),
                _ => 0.0,
            };
            (far_end, length)
        }
    };
    Some(LaserBeam {
        far_end,
        length,
        effect_length,
    })
}

/// The length of the longest of the three axes (the columns of the rotation part) of a bullet
/// transform: 1 for an unscaled bullet, the stretch for a beam.
pub fn longest_axis(transform: &[f32; BULLET_TRANSFORM_FLOATS]) -> f32 {
    (0..AXES)
        .map(|column| {
            (0..AXES)
                .map(|row| transform[row * TRANSFORM_ROW_FLOATS + column].powi(2))
                .sum::<f32>()
                .sqrt()
        })
        .fold(0.0, f32::max)
}

pub fn beam_end_text(far_end: BeamEnd) -> &'static str {
    match far_end {
        BeamEnd::SpawnedAsLaser => "a bullet spawned as a laser",
        BeamEnd::PlainScriptBullet => "a plain script bullet",
        BeamEnd::OtherActor => "an actor that is not a script bullet",
        BeamEnd::Gone => "nothing (its senior is gone)",
    }
}

fn first_beam_line(enemy: EnemyId, effect_id: i32, beam: LaserBeam, axis: f32) -> String {
    format!(
        "enemy bullets: first laser beam from enemy {}:{}: effect {effect_id} stretched to {}, {:.1} units long over an effect length of {:.2} (longest axis {axis:.1})",
        enemy.group,
        enemy.ordinal,
        beam_end_text(beam.far_end),
        beam.length,
        beam.effect_length
    )
}

fn position_of<M: GameMemory>(memory: &M, actor: u64) -> Option<[f32; AXES]> {
    let block = memory
        .read_array::<ACTOR_TRANSFORM_BLOCK_BYTES>(actor + ACTOR_TRANSFORM_BLOCK_OFFSET)
        .ok()?;
    let float_at = |index: usize| {
        let at = index * FLOAT_BYTES;
        f32::from_le_bytes([block[at], block[at + 1], block[at + 2], block[at + 3]])
    };
    Some(core::array::from_fn(|row| {
        float_at(row * TRANSFORM_ROW_FLOATS + TRANSLATION_COLUMN)
    }))
}

fn distance(from: [f32; AXES], to: [f32; AXES]) -> f32 {
    from.iter()
        .zip(to)
        .map(|(near, far)| (far - near).powi(2))
        .sum::<f32>()
        .sqrt()
}
