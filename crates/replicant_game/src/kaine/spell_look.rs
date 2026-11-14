use core::ffi::CStr;
use std::collections::HashSet;

use crate::game::GameError;
use crate::kaine::{SizeBasis, SpellBrightnessGroup, SpellSizeGroup, kept_brightness, kept_size};
use crate::memory::GameMemory;

pub const PLAYER_SPELL_EFFECT_PREFIX: &str = "efP_black";
pub const PLAYER_SPELL_KINDS: [&str; 8] = [
    "Bullet",
    "Spear",
    "Execution",
    "Glutton",
    "Hand",
    "Wall",
    "Rotation",
    "Illusion",
];
pub const SWAPPED_BODY_SPELL_TINT_RGB: [f32; 3] = [0.3, 0.55, 1.0];
pub const SWAPPED_SPELL_EFFECT_MAX_LIFE_MS: u64 = 8_000;
pub const MAX_TRACKED_SWAPPED_SPELL_EFFECTS: usize = 256;
pub const SPELL_EFFECT_NAME_READ_LEN: usize = 64;
const HER_SPAWN_EFFECT_COLOUR_STRENGTH: f32 = 0.5;
const HER_HIT_EFFECT_COLOUR_STRENGTH: f32 = 0.5;

/// How sure a pair is before a run: `ByRole` when both names say the same part (a shot, its hit),
/// `Guess` when the role of her effect is read from its name only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairBasis {
    ByRole,
    Guess,
}

/// One of the player's spell effects and the effect of hers that is drawn instead on the swapped
/// body. Ids are the effect number table's (`None` where the game spawns the effect by name only).
/// `size` multiplies her effect's drawn scale so it matches the one it replaces; `brightness`
/// multiplies her effect's colour when it has a `brightness_group` (made for one large cast, it
/// is drawn here many times a second).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpellLookPair {
    pub original: &'static str,
    pub original_id: Option<i32>,
    pub hers: &'static CStr,
    pub hers_id: i32,
    pub basis: PairBasis,
    pub size: f32,
    pub size_basis: SizeBasis,
    pub size_group: SpellSizeGroup,
    pub brightness: f32,
    pub brightness_group: Option<SpellBrightnessGroup>,
}

#[derive(Debug, Clone, Copy)]
struct HerEffect {
    name: &'static CStr,
    id: i32,
    size: f32,
    size_basis: SizeBasis,
    group: SpellSizeGroup,
    brightness: f32,
    brightness_group: Option<SpellBrightnessGroup>,
}

/// What a spell effect looks like on the swapped body: her effect, the spell's own effect in her
/// colour, or unchanged (not a spell effect, or nothing to swap).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpellLook {
    Hers(&'static SpellLookPair),
    Tinted,
    AsItIs,
}

/// The names already logged, so each spell effect is logged once.
#[derive(Debug, Default)]
pub struct SpellLookLog {
    seen: HashSet<String>,
}

/// Her effects drawn in place of a spell's: one that outlives `SWAPPED_SPELL_EFFECT_MAX_LIFE_MS`
/// is ended, in case hers loops where the spell's did not, and each keeps its pair's size while
/// it lives.
#[derive(Debug, Default)]
pub struct SwappedSpellEffects {
    started: Vec<HerSpellEffect>,
}

#[derive(Debug, Clone, Copy)]
struct HerSpellEffect {
    handle: u32,
    started_ms: u64,
    size: f32,
    written_scale: [f32; 4],
    brightness: f32,
    written_colour: Option<[f32; 3]>,
}

const HER_BALL: HerEffect = her_effect(c"efC_kaine_MagicBall01", 233, 0.35, SpellSizeGroup::Shots);
const HER_BIG_BALL: HerEffect =
    her_effect(c"efC_kaine_MagicBall02", 288, 0.6, SpellSizeGroup::Shots);
const HER_BALL_START: HerEffect = her_effect(
    c"efC_kaine_MagicBallStart02",
    289,
    0.4,
    SpellSizeGroup::Shots,
)
.dimmed(
    SpellBrightnessGroup::Spawns,
    HER_SPAWN_EFFECT_COLOUR_STRENGTH,
);
const HER_BALL_HIT: HerEffect =
    her_effect(c"efC_kaine_MagicBallHit01", 588, 0.4, SpellSizeGroup::Hits)
        .dimmed(SpellBrightnessGroup::Hits, HER_HIT_EFFECT_COLOUR_STRENGTH);
const HER_BIG_BALL_HIT: HerEffect =
    her_effect(c"efC_kaine_MagicBallHit02", 290, 0.6, SpellSizeGroup::Hits)
        .dimmed(SpellBrightnessGroup::Hits, HER_HIT_EFFECT_COLOUR_STRENGTH);
const HER_ARRAY: HerEffect =
    her_effect(c"efC_kaine_MagicArray01", 230, 0.8, SpellSizeGroup::Circles);
const HER_ARRAY_START: HerEffect = her_effect(
    c"efC_kaine_MagicArrayStart01",
    234,
    0.8,
    SpellSizeGroup::Circles,
);
const HER_MAGIC_HIT: HerEffect =
    her_effect(c"efC_kaine_MagicHit01", 9821, 0.7, SpellSizeGroup::Hits)
        .dimmed(SpellBrightnessGroup::Hits, HER_HIT_EFFECT_COLOUR_STRENGTH);
const HER_SHOCK_WAVE: HerEffect =
    her_effect(c"efC_kaine_ShockWave01", 225, 0.8, SpellSizeGroup::Circles);
const HER_TRACE: HerEffect = her_effect(c"efC_kaine_Trace01", 271, 0.7, SpellSizeGroup::Shots);
const HER_TRACE_BIRTH: HerEffect =
    her_effect(c"efC_kaine_TraceBirth01", 272, 0.7, SpellSizeGroup::Shots);
const HER_TRACE_START: HerEffect =
    her_effect(c"efC_kaine_TraceStart01", 7007, 0.7, SpellSizeGroup::Shots).dimmed(
        SpellBrightnessGroup::Spawns,
        HER_SPAWN_EFFECT_COLOUR_STRENGTH,
    );
const HER_TRACE_HIT: HerEffect =
    her_effect(c"efC_kaine_TraceHit01", 7023, 0.7, SpellSizeGroup::Hits)
        .dimmed(SpellBrightnessGroup::Hits, HER_HIT_EFFECT_COLOUR_STRENGTH);

const fn her_effect(
    name: &'static CStr,
    id: i32,
    guessed_size: f32,
    group: SpellSizeGroup,
) -> HerEffect {
    HerEffect {
        name,
        id,
        size: guessed_size,
        size_basis: SizeBasis::Guess,
        group,
        brightness: 1.0,
        brightness_group: None,
    }
}

const fn pair(
    original: &'static str,
    original_id: Option<i32>,
    hers: HerEffect,
    basis: PairBasis,
) -> SpellLookPair {
    SpellLookPair {
        original,
        original_id,
        hers: hers.name,
        hers_id: hers.id,
        basis,
        size: hers.size,
        size_basis: hers.size_basis,
        size_group: hers.group,
        brightness: hers.brightness,
        brightness_group: hers.brightness_group,
    }
}

pub const SPELL_LOOK_PAIRS: [SpellLookPair; 54] = [
    pair(
        "efP_blackBulletBullet01",
        Some(47),
        HER_BALL,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackBullet1stShot01",
        Some(245),
        HER_BALL,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackBulletChargeBullet01",
        Some(51),
        HER_BIG_BALL,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackBulletMuzzle01",
        None,
        HER_BALL_START,
        PairBasis::Guess,
    ),
    pair(
        "efP_blackBullet1stShotMuzzle01",
        None,
        HER_BALL_START,
        PairBasis::Guess,
    ),
    pair(
        "efP_blackBulletChargeMuzzle01",
        None,
        HER_BALL_START,
        PairBasis::Guess,
    ),
    pair(
        "efP_blackBulletHit01",
        Some(192),
        HER_BALL_HIT,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackBulletHitWall01",
        Some(193),
        HER_BALL_HIT,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackBulletJustHit01",
        Some(9903),
        HER_BALL_HIT,
        PairBasis::Guess,
    ),
    pair(
        "efP_blackBulletExtinct01",
        Some(197),
        HER_BALL_HIT,
        PairBasis::Guess,
    ),
    pair(
        "efP_blackBulletChargeHit01",
        Some(9902),
        HER_BIG_BALL_HIT,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackBulletChargeHitWall01",
        Some(9906),
        HER_BIG_BALL_HIT,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackBulletChargeExtinct01",
        Some(9905),
        HER_BIG_BALL_HIT,
        PairBasis::Guess,
    ),
    pair(
        "efP_blackBulletChargeTrail01",
        Some(196),
        HER_TRACE,
        PairBasis::Guess,
    ),
    pair("efP_blackSpearBullet01", None, HER_TRACE, PairBasis::Guess),
    pair("efP_blackSpearBullet11", None, HER_TRACE, PairBasis::Guess),
    pair(
        "efP_blackSpearMuzzle01",
        None,
        HER_TRACE_START,
        PairBasis::Guess,
    ),
    pair(
        "efP_blackSpearMuzzle11",
        None,
        HER_TRACE_START,
        PairBasis::Guess,
    ),
    pair(
        "efP_blackSpearStagnation01",
        None,
        HER_TRACE_BIRTH,
        PairBasis::Guess,
    ),
    pair(
        "efP_blackSpearStagnation11",
        None,
        HER_TRACE_BIRTH,
        PairBasis::Guess,
    ),
    pair(
        "efP_blackSpearHit01",
        None,
        HER_TRACE_HIT,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackSpearHit11",
        None,
        HER_TRACE_HIT,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackSpearWallHit01",
        None,
        HER_TRACE_HIT,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackSpearExtinct01",
        Some(186),
        HER_MAGIC_HIT,
        PairBasis::Guess,
    ),
    pair(
        "efP_blackSpearVanish01",
        None,
        HER_MAGIC_HIT,
        PairBasis::Guess,
    ),
    pair(
        "efP_blackExecution_mc01_st",
        None,
        HER_ARRAY_START,
        PairBasis::Guess,
    ),
    pair("efP_blackExecution_mc01", None, HER_ARRAY, PairBasis::Guess),
    pair("efP_blackExecution_mc02", None, HER_ARRAY, PairBasis::Guess),
    pair("efP_blackExecution_mc03", None, HER_ARRAY, PairBasis::Guess),
    pair("efP_blackExecution_mc04", None, HER_ARRAY, PairBasis::Guess),
    pair(
        "efP_blackExecutionHit01",
        None,
        HER_MAGIC_HIT,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackGluttonMagicCircle01",
        None,
        HER_ARRAY,
        PairBasis::Guess,
    ),
    pair(
        "efP_blackGluttonMagicCircle02",
        None,
        HER_ARRAY,
        PairBasis::Guess,
    ),
    pair(
        "efP_blackGluttonMagicCircle03",
        None,
        HER_ARRAY,
        PairBasis::Guess,
    ),
    pair(
        "efP_blackGluttonMuzzle01",
        Some(9910),
        HER_BALL_START,
        PairBasis::Guess,
    ),
    pair(
        "efP_blackGluttonMuzzle02",
        Some(9911),
        HER_BALL_START,
        PairBasis::Guess,
    ),
    pair(
        "efP_blackGluttonMuzzle03",
        Some(9912),
        HER_BALL_START,
        PairBasis::Guess,
    ),
    pair(
        "efP_blackGluttonMuzzle04",
        Some(9913),
        HER_BALL_START,
        PairBasis::Guess,
    ),
    pair(
        "efP_blackGluttonMuzzle05",
        Some(9914),
        HER_BALL_START,
        PairBasis::Guess,
    ),
    pair(
        "efP_blackGluttonShot01",
        Some(9915),
        HER_BIG_BALL,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackGluttonShot02",
        Some(9916),
        HER_BIG_BALL,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackGluttonShot03",
        Some(9917),
        HER_BIG_BALL,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackGluttonShot04",
        Some(9918),
        HER_BIG_BALL,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackGluttonShot05",
        Some(9919),
        HER_BIG_BALL,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackGluttonShotHit01",
        Some(9924),
        HER_BIG_BALL_HIT,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackGluttonShotHit02",
        Some(9925),
        HER_BIG_BALL_HIT,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackGluttonShotHit03",
        Some(9926),
        HER_BIG_BALL_HIT,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackGluttonShotHit04",
        Some(9927),
        HER_BIG_BALL_HIT,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackGluttonShotHit05",
        Some(9928),
        HER_BIG_BALL_HIT,
        PairBasis::ByRole,
    ),
    pair("efP_blackHandHit01", None, HER_MAGIC_HIT, PairBasis::ByRole),
    pair(
        "efP_blackHandChargeHit01",
        None,
        HER_MAGIC_HIT,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackHandJGHit01",
        None,
        HER_MAGIC_HIT,
        PairBasis::ByRole,
    ),
    pair(
        "efP_blackHandWave01",
        None,
        HER_SHOCK_WAVE,
        PairBasis::Guess,
    ),
    pair(
        "efP_blackHandChargeWave01",
        None,
        HER_SHOCK_WAVE,
        PairBasis::Guess,
    ),
];

impl HerEffect {
    const fn dimmed(self, group: SpellBrightnessGroup, guessed_strength: f32) -> Self {
        Self {
            brightness: guessed_strength,
            brightness_group: Some(group),
            ..self
        }
    }
}

impl SpellLookPair {
    pub fn her_name(&self) -> &'static str {
        self.hers.to_str().unwrap_or_default()
    }
}

impl SpellLookLog {
    pub fn is_first(&mut self, name: &str) -> bool {
        self.seen.insert(name.to_owned())
    }
}

impl SwappedSpellEffects {
    /// Watches her effect `handle`, spawned at `spawned_scale` with the pair's `size` in it, its
    /// colour to be drawn at `brightness`.
    pub fn note(
        &mut self,
        handle: u32,
        now_ms: u64,
        size: f32,
        spawned_scale: f32,
        brightness: f32,
    ) {
        if self.started.len() >= MAX_TRACKED_SWAPPED_SPELL_EFFECTS {
            self.started.remove(0);
        }
        self.started.push(HerSpellEffect {
            handle,
            started_ms: now_ms,
            size,
            written_scale: [spawned_scale; 4],
            brightness,
            written_colour: None,
        });
    }

    pub fn take_overdue(&mut self, now_ms: u64) -> Vec<u32> {
        let (overdue, live): (Vec<_>, Vec<_>) = self.started.iter().partition(|effect| {
            now_ms.saturating_sub(effect.started_ms) >= SWAPPED_SPELL_EFFECT_MAX_LIFE_MS
        });
        self.started = live;
        overdue.into_iter().map(|effect| effect.handle).collect()
    }

    /// The scales to write so every watched effect keeps its size after the game set its own:
    /// `drawn` reads an effect's current scale (None while it has no instance).
    pub fn keep_sizes(
        &mut self,
        mut drawn: impl FnMut(u32) -> Option<[f32; 4]>,
    ) -> Vec<(u32, [f32; 4])> {
        let mut writes = Vec::new();
        for effect in &mut self.started {
            if let Some(scale) = drawn(effect.handle)
                && let Some(kept) = kept_size(scale, effect.written_scale, effect.size)
            {
                effect.written_scale = kept;
                writes.push((effect.handle, kept));
            }
        }
        writes
    }

    /// The colours to write so every watched effect is drawn at its brightness, also after the
    /// game colours it (a styled hit): `drawn` reads an effect's current colour.
    pub fn keep_brightness(
        &mut self,
        mut drawn: impl FnMut(u32) -> Option<[f32; 3]>,
    ) -> Vec<(u32, [f32; 3])> {
        let mut writes = Vec::new();
        for effect in &mut self.started {
            if let Some(colour) = drawn(effect.handle)
                && let Some(kept) =
                    kept_brightness(colour, effect.written_colour, effect.brightness)
            {
                effect.written_colour = Some(kept);
                writes.push((effect.handle, kept));
            }
        }
        writes
    }

    pub fn len(&self) -> usize {
        self.started.len()
    }

    pub fn is_empty(&self) -> bool {
        self.started.is_empty()
    }
}

pub fn spell_kind(name: &str) -> Option<&'static str> {
    let rest = name.strip_prefix(PLAYER_SPELL_EFFECT_PREFIX)?;
    Some(
        PLAYER_SPELL_KINDS
            .iter()
            .find(|kind| rest.starts_with(**kind))
            .copied()
            .unwrap_or("other"),
    )
}

pub fn spell_look(name: &str) -> SpellLook {
    if let Some(found) = SPELL_LOOK_PAIRS.iter().find(|pair| pair.original == name) {
        return SpellLook::Hers(found);
    }
    if name.starts_with(PLAYER_SPELL_EFFECT_PREFIX) {
        SpellLook::Tinted
    } else {
        SpellLook::AsItIs
    }
}

/// The look of an effect spawned on this side: only the local player's spells on the swapped
/// body change, never an effect played for the partner.
pub fn swapped_spell_look(name: &str, wears_swapped_body: bool, for_partner: bool) -> SpellLook {
    if !wears_swapped_body || for_partner {
        return SpellLook::AsItIs;
    }
    spell_look(name)
}

/// The name sent to the partner for an effect of the local player's: hers when it has a pair.
pub fn sent_spell_effect_name(name: String, wears_swapped_body: bool) -> String {
    match swapped_spell_look(&name, wears_swapped_body, false) {
        SpellLook::Hers(found) => found.her_name().to_owned(),
        SpellLook::Tinted | SpellLook::AsItIs => name,
    }
}

pub fn her_effect_id(effect_id: i32) -> Option<i32> {
    SPELL_LOOK_PAIRS
        .iter()
        .find(|pair| pair.original_id == Some(effect_id))
        .map(|pair| pair.hers_id)
}

/// A shot's four effect ids as sent to the partner: each with a pair becomes hers.
pub fn sent_shot_effect_ids<const SLOTS: usize>(
    effect_ids: [i32; SLOTS],
    wears_swapped_body: bool,
) -> [i32; SLOTS] {
    if !wears_swapped_body {
        return effect_ids;
    }
    effect_ids.map(|id| her_effect_id(id).unwrap_or(id))
}

pub fn tint_style(style: &mut [f32]) {
    for (channel, value) in style.iter_mut().zip(SWAPPED_BODY_SPELL_TINT_RGB) {
        *channel = value;
    }
}

pub fn spell_look_line(name: &str, look: SpellLook) -> String {
    let kind = spell_kind(name).unwrap_or("other");
    match look {
        SpellLook::Hers(found) => {
            let original_id = found
                .original_id
                .map_or_else(|| "by name".to_owned(), |id| format!("id {id}"));
            let basis = match found.basis {
                PairBasis::ByRole => "same role",
                PairBasis::Guess => "a guess",
            };
            let size_basis = match found.size_basis {
                SizeBasis::FromData => "from data",
                SizeBasis::Guess => "a guess",
            };
            let colour = match found.brightness_group {
                Some(group) => format!(
                    ", colour x{:.2} ({}, a guess)",
                    found.brightness,
                    group.label()
                ),
                None => String::new(),
            };
            format!(
                "spell look: {kind} spell effect {name} ({original_id}) -> {} (id {}), {basis}, size x{:.2} ({size_basis}){colour}",
                found.her_name(),
                found.hers_id,
                found.size
            )
        }
        SpellLook::Tinted => format!(
            "spell look: {kind} spell effect {name} has no pair -> kept, in her colour when spawned by name"
        ),
        SpellLook::AsItIs => format!("spell look: {name} kept as it is"),
    }
}

/// The name of an effect spawned by name when it is one of the player's spell effects; None
/// after reading only the prefix otherwise.
pub fn read_spell_effect_name<M: GameMemory>(
    memory: &M,
    address: u64,
) -> Result<Option<String>, GameError> {
    if address == 0 {
        return Ok(None);
    }
    let mut prefix = [0_u8; PLAYER_SPELL_EFFECT_PREFIX.len()];
    memory.read(address, &mut prefix)?;
    if prefix != *PLAYER_SPELL_EFFECT_PREFIX.as_bytes() {
        return Ok(None);
    }
    let mut bytes = Vec::with_capacity(SPELL_EFFECT_NAME_READ_LEN);
    for offset in 0..SPELL_EFFECT_NAME_READ_LEN as u64 {
        match memory.read_u8(address + offset)? {
            0 => return Ok(String::from_utf8(bytes).ok()),
            byte => bytes.push(byte),
        }
    }
    Ok(None)
}
