mod companion_off;
mod companion_off_fallback;
mod companion_off_fights;
mod companion_off_fights_from_scripts;
mod hit_power;
mod level_in_use;
mod partner_hide;
mod reference_weapon;
mod row_origin;
mod save_guard;
mod scene_stand_in;
mod scene_stand_ins;
mod scene_stand_ins_from_scripts;
mod spell_brightness;
mod spell_effect_tally;
mod spell_look;
mod spell_size;
mod stand_in;
mod stand_in_beats;
mod stats;
mod swapped_level;

pub use companion_off::{
    CompanionCallLend, CompanionLendReason, CompanionOff, CompanionOffEvent, CompanionOffReport,
    CompanionOffStep, CompanionOffTick,
};
pub use companion_off_fallback::{CompanionOffBossSigns, CompanionOffFallback};
pub use companion_off_fights::{
    ApproachFlags, COMPANION_OFF_FIGHTS, CompanionOffApproach, CompanionOffFight,
    companion_off_fight_opened_by, companion_off_fight_started_by, companion_off_fights_approached,
    companion_off_fights_approached_in, companion_off_fights_from_scripts_in_use,
    fight_from_scripts_opened_by, rows_no_hand_made_fight_covers,
};
pub use companion_off_fights_from_scripts::COMPANION_OFF_FIGHTS_FROM_SCRIPTS;

pub use hit_power::{
    HeldWeapon, PERCENT_PER_WHOLE, SWAPPED_POWER_LINE_INTERVAL_MS, SwappedBodyPower,
    SwappedHitTally, SwappedPowerLog, WeaponPercents, read_minimum_damages, swapped_power_line,
    weapon_factor, weapon_powered_source,
};
pub use level_in_use::{LevelFields, level_fields_in_use};
pub use partner_hide::{PartnerHideStep, PartnerKaineHide, partner_kaine_hidden_wanted};
pub use reference_weapon::{ReferenceSource, ReferenceWeapon};
pub use row_origin::RowOrigin;
pub use save_guard::{KaineCopyPutBack, put_back_kaine_copies};
pub use scene_stand_in::{GuestSceneStandIn, SceneStandInEvent, SceneStandInFacts};
pub use scene_stand_ins::{
    SCENE_STAND_INS, ScenePlace, ScenePlaceSource, SceneStandIn, scene_stand_in_for_talk,
    scene_stand_in_opened_by, scene_stand_in_place, scene_stand_ins_from_scripts_in_use,
};
pub use scene_stand_ins_from_scripts::SCENE_STAND_INS_FROM_SCRIPTS;
pub use spell_brightness::{
    EFFECT_STYLE_COLOUR_INDICES, MAX_SWAPPED_SPELL_EFFECT_COLOUR, SPELL_BRIGHTNESS_GROUPS,
    SPELL_BRIGHTNESS_SLIDER_MAX, SPELL_BRIGHTNESS_SLIDER_MIN, SpellBrightnessGroup,
    SpellBrightnessSliders, kept_brightness, set_style_colour, spell_effect_brightness,
    style_colour,
};
pub use spell_effect_tally::{
    HER_EFFECT_ROLES, HerEffectRole, SPELL_EFFECT_TALLY_INTERVAL_MS, SpellEffectTally,
    her_effect_role,
};
pub use spell_look::{
    MAX_TRACKED_SWAPPED_SPELL_EFFECTS, PLAYER_SPELL_EFFECT_PREFIX, PLAYER_SPELL_KINDS, PairBasis,
    SPELL_EFFECT_NAME_READ_LEN, SPELL_LOOK_PAIRS, SWAPPED_BODY_SPELL_TINT_RGB,
    SWAPPED_SPELL_EFFECT_MAX_LIFE_MS, SpellLook, SpellLookLog, SpellLookPair, SwappedSpellEffects,
    her_effect_id, read_spell_effect_name, sent_shot_effect_ids, sent_spell_effect_name,
    spell_kind, spell_look, spell_look_line, swapped_spell_look, tint_style,
};
pub use spell_size::{
    EFFECT_STYLE_SCALE_INDICES, MAX_SWAPPED_SPELL_EFFECT_SCALE, SPELL_SIZE_GROUPS,
    SPELL_SIZE_SLIDER_MAX, SPELL_SIZE_SLIDER_MIN, SizeBasis, SpellSizeGroup, SpellSizeSliders,
    kept_size, set_style_scale, spell_effect_size, style_scale,
};
pub use stand_in::{
    GuestStandIn, HeldTrap, OwnTrapVerdict, StandInEvent, StandInFacts, StandInReport, StandInSide,
};
pub use stand_in_beats::{
    STAND_IN_BEATS, StandInBeat, open_stand_in_beat, stand_in_beat_moved_after, stand_in_hp_damage,
};
pub use stats::{
    KaineStatsSynced, clamped_current, level_row_in_range, param_patches, sync_kaine_stats,
};
pub use swapped_level::{
    SwappedBodyLevel, SwappedLevelChange, SwappedLevelStep, swapped_level_line,
};

use crate::offsets::SAVE_FORCED_WEAPON_FLAG_OFFSET;

pub const KAINE_PLAYER_FLAG_SAVE_OFFSET: usize = SAVE_FORCED_WEAPON_FLAG_OFFSET as usize;
pub const KAINE_COPY_SAVE_START: usize = KAINE_PLAYER_FLAG_SAVE_OFFSET;
pub const KAINE_COPY_SAVE_END: usize = 0x1161;
pub const SEED_INDEX_SAVE_OFFSET: usize = 0x28;
pub const KAINE_PLAYER_FLAG_SET: i32 = 1;
pub const PARTNER_EXECUTES_FLAG_OFFSET: u64 = 0x13E4;
pub const KAINE_LEVEL_SAVE_OFFSET: usize = 0xE3C;
pub const KAINE_EXPERIENCE_SAVE_OFFSET: usize = 0xE40;
pub const COMPANION_GAUGE_MODE_OFFSET: u64 = 0x1_5E94;
pub const HUD_PARTNER_GAUGE_OFFSET: u64 = 0x288;
pub const PARTNER_GAUGE_ACTOR_HANDLE_OFFSET: u64 = 0x70;
pub const GUEST_HIT_SCALE_IN_A_STAND_IN_BEAT: f32 = 0.25;
pub const HELD_TRAP_RELEASE_DELAY_MS: u64 = 3_000;
pub const SCENE_STAND_IN_MAX_OPEN_MS: u64 = 180_000;
pub const COMPANION_PLACE_TRUSTED_WITHIN_UNITS: f32 = 3.0;
pub const COMPANION_LENT_AFTER_A_CALL_MS: u64 = 3_000;
pub const COMPANION_LENT_AFTER_A_SCENE_MS: u64 = 3_000;
pub const COMPANION_OFF_GAUGE_CLOSED_FOR_GOOD_MS: u64 = 20_000;
pub const COMPANION_OFF_BOSS_GONE_FOR_GOOD_MS: u64 = 20_000;
pub const COMPANION_OFF_BOSS_NEVER_CAME_MS: u64 = 120_000;
pub const LEVEL_STATS_TABLE_RVA: u32 = 0x47C_2638;
pub const LEVEL_STATS_ROW_BYTES: usize = 0x2C;
pub const MAX_LEVEL_STATS_ROW: i32 = 0x62;
pub const STATS_ROW_TO_PARAM: [(usize, u64); 9] = [
    (0x04, 0x18),
    (0x08, 0x1C),
    (0x0C, 0x3C),
    (0x10, 0x40),
    (0x14, 0x44),
    (0x18, 0x48),
    (0x1C, 0x30),
    (0x20, 0x4C),
    (0x24, 0x50),
];
