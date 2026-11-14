use std::sync::Mutex;
use std::sync::PoisonError;
use std::time::Instant;

use replicant_game::marker::read_world_flags;
use replicant_game::offsets::BODY_MOTION_LAYER;
use replicant_game::puppet::{
    LayerMotion, motion_name, read_layer_motion, read_ridden_boar, read_weapon_look,
};
use replicant_game::readiness::{REPORT_MIN_INTERVAL_MS, classify, read_play_checks};
use replicant_game::{PhaseEntry, Replicant};
use replicant_proto::{NO_MOTION_NAME_HASH, PartnerActivity, RiddenBoar, motion_name_hash};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::memory::InProcessMemory;
use crate::net::{AutoConfig, LocalSnapshot, SEND_INTERVAL, handle, start_link};

static LAST_PUBLISHED_AT: Mutex<Option<Instant>> = Mutex::new(None);
static LAST_SENT_ACTIVITY: Mutex<Option<PartnerActivity>> = Mutex::new(None);
static LAST_REPORTED_AT: Mutex<Option<Instant>> = Mutex::new(None);
static LAST_SENT_RIDING: Mutex<bool> = Mutex::new(false);

pub fn report_readiness(build: &VerifiedBuild) {
    let Some(net) = handle() else {
        return;
    };
    let now = Instant::now();
    let mut last = LAST_REPORTED_AT
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if last.is_some_and(|reported| {
        now.saturating_duration_since(reported).as_millis() < u128::from(REPORT_MIN_INTERVAL_MS)
    }) {
        return;
    }
    *last = Some(now);
    let game = Replicant::new(InProcessMemory::new(build));
    net.report_readiness(classify(read_play_checks(&game)));
}

pub fn quit_when_asked() {
    let Some(net) = handle() else {
        return;
    };
    if net.take_quit_request() {
        log_line("net: closing the game window because the launcher asked");
        crate::overlay::close_game_window();
    }
}

pub fn publish_local_snapshot(build: &VerifiedBuild) {
    let Some(net) = handle() else {
        return;
    };
    if !net.wants_snapshots() {
        return;
    }
    let now = Instant::now();
    let mut last = LAST_PUBLISHED_AT
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if last.is_some_and(|published| now.saturating_duration_since(published) < SEND_INTERVAL) {
        return;
    }
    let game = Replicant::new(InProcessMemory::new(build));
    if let Some(snapshot) = read_snapshot(&game) {
        *last = Some(now);
        crate::player_names::log_local_name_once(build);
        net.publish(snapshot);
    }
}

pub fn start_auto() {
    crate::frame_tuning::load_frame_tuning();
    if let Ok(Some(config)) = AutoConfig::load_beside_game() {
        crate::event_log::set_event_log_enabled(config.event_log_enabled());
        crate::kaine::set_companion_rows_from_scripts(config.companion_rows_from_scripts_enabled());
    }
    match AutoConfig::load_beside_game() {
        Ok(Some(AutoConfig {
            link: Some(port), ..
        })) => {
            log_line(&format!("net: launcher link on port {port}"));
            start_link(port);
        }
        Ok(Some(_)) => log_line("net: nier-coop.toml has no link port, co-op is off"),
        Ok(None) => log_line("net: no nier-coop.toml, co-op is off"),
        Err(error) => log_line(&format!("net: {error}")),
    }
}

fn read_snapshot(game: &Replicant<InProcessMemory>) -> Option<LocalSnapshot> {
    let phase = u16::try_from(PhaseEntry::index_of(&game.current_phase().ok()?)?).ok()?;
    let position = game.position().ok()?;
    let actor = game.playable_actor().ok();
    let motion = actor
        .and_then(|actor| read_layer_motion(game.memory(), actor, BODY_MOTION_LAYER).ok())
        .flatten();
    let weapon = actor.and_then(|actor| read_weapon_look(game.memory(), actor).ok().flatten());
    Some(LocalSnapshot {
        phase,
        position: [position.x, position.y, position.z],
        yaw_radians: game.yaw_radians().ok()?,
        hp: game.health().ok()?,
        max_hp: game.max_health().ok()?,
        body_motion: motion,
        body_motion_hash: body_motion_hash(game, actor, motion),
        weapon,
        activity: sent_activity(game, phase),
        mp: game.magic().unwrap_or(0.0),
        max_mp: game.max_magic().unwrap_or(0.0),
        level: game.level().unwrap_or(0),
        ridden_boar: sent_ridden_boar(game, actor),
    })
}

fn sent_ridden_boar(game: &Replicant<InProcessMemory>, actor: Option<u64>) -> Option<RiddenBoar> {
    // Any unreadable or out-of-range part sends no boar: the partner then sees a rider alone.
    let boar = actor
        .and_then(|actor| read_ridden_boar(game.memory(), actor).ok().flatten())
        .filter(|boar| boar.validate().is_ok());
    let mut last = LAST_SENT_RIDING
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if *last != boar.is_some() {
        *last = boar.is_some();
        match boar {
            Some(boar) => log_line(&format!(
                "ridden boar: you ride, sending your boar (offset {:?}, turn {:.2}, motion {:?})",
                boar.offset_from_rider,
                boar.yaw_from_rider_radians,
                boar.motion.map(|motion| motion.index)
            )),
            None => log_line("ridden boar: you no longer ride, no boar is sent"),
        }
    }
    boar
}

fn body_motion_hash(
    game: &Replicant<InProcessMemory>,
    actor: Option<u64>,
    motion: Option<LayerMotion>,
) -> u32 {
    let (Some(actor), Some(motion)) = (actor, motion) else {
        return NO_MOTION_NAME_HASH;
    };
    motion_name(game.memory(), actor, motion.index)
        .ok()
        .flatten()
        .map_or(NO_MOTION_NAME_HASH, |name| motion_name_hash(&name))
}

fn sent_activity(game: &Replicant<InProcessMemory>, phase: u16) -> PartnerActivity {
    let flags = read_world_flags(game.memory()).ok();
    let activity = flags
        .map(crate::dialogue::local_activity)
        .unwrap_or_default();
    let mut last = LAST_SENT_ACTIVITY
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if *last != Some(activity) {
        log_line(&format!(
            "activity: sending {activity:?} (was {:?}, main menu open {}, control mask {:#x}, event scene {}, phase {phase})",
            *last,
            flags.is_some_and(|flags| flags.main_menu_open),
            flags.map_or(0, |flags| flags.control_mask),
            flags.is_some_and(|flags| flags.event_scene),
        ));
        *last = Some(activity);
    }
    activity
}
