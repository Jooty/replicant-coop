use core::ffi::c_void;
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::Instant;

use replicant_game::Replicant;
use replicant_game::kaine::{
    ReferenceWeapon, SwappedBodyLevel, SwappedLevelChange, clamped_current, swapped_level_line,
};
use replicant_game::offsets::{
    PLAYER_PARAM_ADD_EXPERIENCE_FUNCTION_RVA, PLAYER_PARAM_ARRAY_RVA,
    PLAYER_PARAM_INIT_AT_LEVEL_FUNCTION_RVA,
};
use replicant_game::prologue_levels::{LevelCatchUp, LocalLevel, PartnerLevel};
use replicant_proto::{Message, NO_WEAPON_REPORTED, PartnerBody, PrologueLevel};

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::kaine::host_weapon_reported;
use crate::memory::InProcessMemory;
use crate::net;
use crate::player_body::real_body;

static CATCH_UP: LazyLock<Mutex<LevelCatchUp>> = LazyLock::new(Mutex::default);
static SWAPPED_LEVEL: LazyLock<Mutex<SwappedBodyLevel>> = LazyLock::new(Mutex::default);
static LAST_SENT_WEAPON: Mutex<Option<ReferenceWeapon>> = Mutex::new(None);
static EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);

const SHOW_LEVEL_UP: u8 = 1;
const KEEP_THE_SAVE_EXPERIENCE: u8 = 0;
const KEEP_CURRENT_HEALTH: u8 = 0;

type AddExperience = unsafe extern "system" fn(*mut c_void, i32, u8);
type InitAtLevel = unsafe extern "system" fn(*mut c_void, i32, u8, u8);

pub fn tick(build: &VerifiedBuild) {
    let Some(net) = net::handle() else {
        return;
    };
    let mut catch_up = CATCH_UP.lock().unwrap_or_else(PoisonError::into_inner);
    let mut swapped = SWAPPED_LEVEL.lock().unwrap_or_else(PoisonError::into_inner);
    for message in net.take_relayed(|message| matches!(message, Message::PrologueLevel(_))) {
        let Message::PrologueLevel(reported_message) = message else {
            continue;
        };
        let reported = PartnerLevel {
            level: reported_message.level,
            experience: reported_message.experience,
        };
        swapped.host_reported(reported);
        host_weapon_reported((reported_message.weapon != NO_WEAPON_REPORTED).then_some(
            ReferenceWeapon {
                id: reported_message.weapon,
                level: i8::try_from(reported_message.weapon_level).unwrap_or(0),
            },
        ));
        let known = catch_up.partner();
        if known.is_none_or(|known| reported.experience >= known.experience) {
            catch_up.partner_reported(reported);
        }
    }
    let mut game = Replicant::new(InProcessMemory::new(build));
    let (Ok(phase), Ok(level), Ok(experience)) =
        (game.current_phase(), game.level(), game.experience())
    else {
        return;
    };
    let now_ms = u64::try_from(EPOCH.elapsed().as_millis()).unwrap_or(u64::MAX);
    let wears_the_swapped_body =
        real_body().is_some_and(|real| real.by_swap && real.body == PartnerBody::SeedKaineE);
    let local = LocalLevel { level, experience };
    if wears_the_swapped_body {
        // Her level only ever follows the host's, so the host is never sent hers to catch up to.
        keep_her_level_at_the_hosts(build, &mut game, &mut swapped, now_ms, local);
        return;
    }
    let step = catch_up.step(now_ms, &phase, local);
    let weapon = ReferenceWeapon::read_equipped(game.memory());
    let weapon_changed = *LAST_SENT_WEAPON
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        != weapon;
    if let Some(sent) = step.send.or(weapon_changed.then_some(local)) {
        *LAST_SENT_WEAPON
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = weapon;
        net.send_relayed(Message::PrologueLevel(PrologueLevel {
            level: sent.level,
            experience: sent.experience,
            weapon: weapon.map_or(NO_WEAPON_REPORTED, |weapon| weapon.id),
            weapon_level: weapon.map_or(0, |weapon| i32::from(weapon.level)),
        }));
    }
    let (Some(gain), Some(partner)) = (step.gain, catch_up.partner()) else {
        return;
    };
    add_experience_through_game(build, gain);
    let after = game.level().unwrap_or(level);
    log_line(&format!(
        "prologue levels: caught up +{gain} experience (local level {level} exp {experience}, partner level {} exp {}, local level now {after})",
        partner.level, partner.experience
    ));
}

fn keep_her_level_at_the_hosts(
    build: &VerifiedBuild,
    game: &mut Replicant<InProcessMemory>,
    swapped: &mut SwappedBodyLevel,
    now_ms: u64,
    her: LocalLevel,
) {
    let step = swapped.step(now_ms, her);
    if let Some((from, to)) = step.changed_by_the_game {
        log_line(&format!(
            "swapped body level: {} (exp {}) -> {} (exp {}) by the guest's own game",
            from.level, from.experience, to.level, to.experience
        ));
    }
    let (Some(change), Some(host)) = (step.change, swapped.host()) else {
        return;
    };
    match change {
        SwappedLevelChange::Raise { gain } => add_experience_through_game(build, gain),
        SwappedLevelChange::BringDown | SwappedLevelChange::SetToHost => {
            init_her_param_at_level(build, host.level);
            if game.set_experience(host.experience).is_err() {
                log_line("swapped body level: cannot write her experience");
            }
            cap_current_health_and_magic(game);
        }
    }
    let after = LocalLevel {
        level: game.level().unwrap_or(her.level),
        experience: game.experience().unwrap_or(her.experience),
    };
    swapped.change_applied(after);
    log_line(&swapped_level_line(change, her, after, host));
}

fn cap_current_health_and_magic(game: &mut Replicant<InProcessMemory>) {
    let (Ok(health), Ok(max_health), Ok(magic), Ok(max_magic)) = (
        game.health(),
        game.max_health(),
        game.magic(),
        game.max_magic(),
    ) else {
        return;
    };
    let (health, magic) = clamped_current(health, max_health, magic, max_magic);
    if game.set_health(health).is_err() || game.set_magic(magic).is_err() {
        log_line("swapped body level: cannot cap her health and magic");
    }
}

#[expect(
    unsafe_code,
    reason = "calls the game's own experience adder at its verified address"
)]
fn add_experience_through_game(build: &VerifiedBuild, amount: i32) {
    let player_param = (build.module_base() + u64::from(PLAYER_PARAM_ARRAY_RVA)) as usize;
    let function =
        (build.module_base() + u64::from(PLAYER_PARAM_ADD_EXPERIENCE_FUNCTION_RVA)) as usize;
    // SAFETY: the RVA starts CPlayerParam::AddExperience (this, amount, redo level-up flag) in the
    // verified build; the array start is slot 0, the local player's CPlayerParam; this runs on the
    // game thread in the script tick.
    unsafe {
        core::mem::transmute::<usize, AddExperience>(function)(
            player_param as *mut c_void,
            amount,
            SHOW_LEVEL_UP,
        );
    }
}

#[expect(
    unsafe_code,
    reason = "calls the game's own player-param init at its verified address"
)]
fn init_her_param_at_level(build: &VerifiedBuild, level: i32) {
    let player_param = (build.module_base() + u64::from(PLAYER_PARAM_ARRAY_RVA)) as usize;
    let function =
        (build.module_base() + u64::from(PLAYER_PARAM_INIT_AT_LEVEL_FUNCTION_RVA)) as usize;
    // SAFETY: the RVA starts CPlayerParam::Init (vtable +0xE0, what the game's level-up calls) in the
    // verified build; slot 0 is the local player's CPlayerParam; this runs on the game thread in
    // the script tick.
    unsafe {
        core::mem::transmute::<usize, InitAtLevel>(function)(
            player_param as *mut c_void,
            level,
            KEEP_THE_SAVE_EXPERIENCE,
            KEEP_CURRENT_HEALTH,
        );
    }
}
