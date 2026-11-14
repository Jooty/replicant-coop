use core::cell::Cell;
use std::sync::{LazyLock, Mutex, PoisonError};

use replicant_game::effects::{finish_effect, read_effect_look, write_effect_look};
use replicant_game::kaine::{
    SWAPPED_SPELL_EFFECT_MAX_LIFE_MS, SpellBrightnessSliders, SpellEffectTally, SpellLook,
    SpellLookLog, SpellSizeSliders, SwappedSpellEffects, her_effect_role, read_spell_effect_name,
    sent_shot_effect_ids, sent_spell_effect_name, set_style_colour, set_style_scale,
    spell_effect_brightness, spell_effect_size, spell_look_line, style_colour, style_scale,
    swapped_spell_look, tint_style,
};
use replicant_proto::PartnerBody;

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::memory::InProcessMemory;
use crate::net::net_clock_ms;
use crate::player_body::real_body;

static LOOK_LOG: LazyLock<Mutex<SpellLookLog>> = LazyLock::new(Mutex::default);
static HER_EFFECTS: LazyLock<Mutex<SwappedSpellEffects>> = LazyLock::new(Mutex::default);
static SIZE_SLIDERS: Mutex<SpellSizeSliders> = Mutex::new(SpellSizeSliders::new());
static BRIGHTNESS_SLIDERS: Mutex<SpellBrightnessSliders> =
    Mutex::new(SpellBrightnessSliders::new());
static EFFECT_TALLY: LazyLock<Mutex<SpellEffectTally>> = LazyLock::new(Mutex::default);

thread_local! {
    static PLAYING_FOR_PARTNER: Cell<bool> = const { Cell::new(false) };
}

/// Runs `call` as playback of the partner's effect: nothing spawned inside takes her look.
pub(crate) fn play_for_partner<R>(call: impl FnOnce() -> R) -> R {
    let was = PLAYING_FOR_PARTNER.with(|flag| flag.replace(true));
    let result = call();
    PLAYING_FOR_PARTNER.with(|flag| flag.set(was));
    result
}

pub(crate) fn wears_swapped_body() -> bool {
    real_body().is_some_and(|real| real.by_swap && real.body == PartnerBody::SeedKaineE)
}

/// The look of an effect about to be spawned by name on this side, logged once per name.
pub(crate) fn spawn_look(build: &VerifiedBuild, name: u64) -> SpellLook {
    if PLAYING_FOR_PARTNER.with(Cell::get) || !wears_swapped_body() {
        return SpellLook::AsItIs;
    }
    let memory = InProcessMemory::new(build);
    let Ok(Some(name)) = read_spell_effect_name(&memory, name) else {
        return SpellLook::AsItIs;
    };
    let look = swapped_spell_look(&name, true, false);
    let first = LOOK_LOG
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_first(&name);
    if first {
        log_line(&spell_look_line(&name, look));
    }
    look
}

/// The factor on the scale her effect is spawned at: the pair's size times its slider; 1 for
/// anything that is not one of her effects.
pub(crate) fn spawn_size(look: SpellLook) -> f32 {
    match look {
        SpellLook::Hers(pair) => spell_effect_size(pair, &spell_size_sliders()),
        SpellLook::Tinted | SpellLook::AsItIs => 1.0,
    }
}

pub(crate) fn spell_size_sliders() -> SpellSizeSliders {
    *SIZE_SLIDERS.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn set_spell_size_sliders(sliders: SpellSizeSliders) {
    *SIZE_SLIDERS.lock().unwrap_or_else(PoisonError::into_inner) = sliders;
}

pub(crate) fn log_spell_size_sliders() {
    log_line(&spell_size_sliders().line());
}

pub(crate) fn spell_brightness_sliders() -> SpellBrightnessSliders {
    *BRIGHTNESS_SLIDERS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn set_spell_brightness_sliders(sliders: SpellBrightnessSliders) {
    *BRIGHTNESS_SLIDERS
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = sliders;
}

pub(crate) fn log_spell_brightness_sliders() {
    log_line(&spell_brightness_sliders().line());
}

/// Counts a shot slot effect that was not sent as a world effect, as the shot sync draws it.
pub(crate) fn note_shot_slot_not_resent() {
    EFFECT_TALLY
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .count_shot_slot_not_resent(net_clock_ms());
}

/// After the spawn: her effect is watched for its life and keeps its size, a spell effect
/// without a pair takes her colour when nothing else will colour it.
pub(crate) fn after_spawn(
    build: &VerifiedBuild,
    look: SpellLook,
    handle: u32,
    outermost: bool,
    size: f32,
    spawned_scale: f32,
) {
    match look {
        SpellLook::Hers(pair) => {
            let now_ms = net_clock_ms();
            let brightness = spell_effect_brightness(pair, &spell_brightness_sliders());
            HER_EFFECTS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .note(handle, now_ms, size, spawned_scale, brightness);
            EFFECT_TALLY
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .count_made(her_effect_role(pair), now_ms);
        }
        SpellLook::Tinted if outermost => {
            let mut memory = InProcessMemory::new(build);
            if let Ok(Some(mut effect_look)) = read_effect_look(&memory, handle) {
                tint_style(&mut effect_look.style);
                let _ = write_effect_look(&mut memory, handle, &effect_look);
            }
        }
        SpellLook::Tinted | SpellLook::AsItIs => {}
    }
}

pub(crate) fn sent_effect_name(name: String) -> String {
    sent_spell_effect_name(name, wears_swapped_body())
}

pub(crate) fn sent_shot_ids<const SLOTS: usize>(effect_ids: [i32; SLOTS]) -> [i32; SLOTS] {
    sent_shot_effect_ids(effect_ids, wears_swapped_body())
}

pub(crate) fn tick_swapped_spell_effects(build: &VerifiedBuild) {
    let mut memory = InProcessMemory::new(build);
    let now_ms = net_clock_ms();
    let (scales, colours, overdue) = {
        let mut effects = HER_EFFECTS.lock().unwrap_or_else(PoisonError::into_inner);
        let scales = effects.keep_sizes(|handle| {
            read_effect_look(&memory, handle)
                .ok()
                .flatten()
                .map(|look| style_scale(&look.style))
        });
        let colours = effects.keep_brightness(|handle| {
            read_effect_look(&memory, handle)
                .ok()
                .flatten()
                .map(|look| style_colour(&look.style))
        });
        (scales, colours, effects.take_overdue(now_ms))
    };
    for (handle, scale) in scales {
        if let Ok(Some(mut look)) = read_effect_look(&memory, handle) {
            set_style_scale(&mut look.style, scale);
            let _ = write_effect_look(&mut memory, handle, &look);
        }
    }
    for (handle, colour) in colours {
        if let Ok(Some(mut look)) = read_effect_look(&memory, handle) {
            set_style_colour(&mut look.style, colour);
            let _ = write_effect_look(&mut memory, handle, &look);
        }
    }
    let tally_line = EFFECT_TALLY
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take_line(now_ms);
    if let Some(line) = tally_line {
        log_line(&line);
    }
    if overdue.is_empty() {
        return;
    }
    let ended = overdue
        .iter()
        .filter(|handle| finish_effect(&mut memory, **handle).unwrap_or(false))
        .count();
    if ended > 0 {
        log_line(&format!(
            "spell look: ended {ended} of her effects still running after {SWAPPED_SPELL_EFFECT_MAX_LIFE_MS} ms"
        ));
    }
}
