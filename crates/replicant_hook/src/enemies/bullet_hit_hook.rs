//! On the host, an enemy bullet's attacks run once more with the guest's copy as the target, so
//! the game's own hit test can find a copy no bullet's target list holds.

use core::ffi::c_void;
use core::ptr;
use std::cell::Cell;
use std::panic;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock, PoisonError};

use replicant_game::GameMemory;
use replicant_game::effects::read_actor_position;
use replicant_game::enemies::ActorHandle;
use replicant_game::enemy_bullets::{
    BulletFirstSightings, PuppetPassInputs, bullet_owner, bullet_reaches_puppet, needs_puppet_pass,
    script_bullet_deals_damage,
};
use replicant_game::offsets::{
    ACTOR_SELF_HANDLE_OFFSET, BULLET_HIT_SPHERE_RADIUS_OFFSET, BULLET_TARGET_GROUP_OFFSET,
    BULLET_TARGET_HANDLE_OFFSET, NORMAL_BULLET_ATTACK_SETTINGS_POINTER_OFFSET,
    NORMAL_BULLET_HIT_RADIUS_GROWTH_RATE_OFFSET, NORMAL_BULLET_RECORD_POINTER_OFFSET,
    NORMAL_BULLET_SETTINGS_EFFECT_SCALE_DIVISOR_OFFSET,
    NORMAL_BULLET_SETTINGS_GROUND_EFFECT_ID_OFFSET, NORMAL_BULLET_SUBMIT_ATTACKS_FUNCTION_RVA,
    SCRIPT_BULLET_BEAM_TO_SENIOR_FLAG_OFFSET, SCRIPT_BULLET_SENIOR_HANDLE_OFFSET,
    SCRIPT_BULLET_SUBMIT_ATTACKS_FUNCTION_RVA,
};
use replicant_proto::EnemyId;

use crate::build_gate::VerifiedBuild;
use crate::diagnostics::log_line;
use crate::enemies::EnemySyncRole;
use crate::enemies::combat_view::{CombatView, PuppetBulletTarget, with_view};
use crate::enemies::enemy_id_text;
use crate::game_hooks::hook_address;
use crate::memory::InProcessMemory;

static HOOKED_BUILD: OnceLock<VerifiedBuild> = OnceLock::new();
static ORIGINAL_SCRIPT_SUBMIT: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static ORIGINAL_NORMAL_SUBMIT: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static LOGGED_BULLETS: LazyLock<Mutex<BulletFirstSightings>> = LazyLock::new(Mutex::default);

thread_local! {
    static IN_SECOND_PASS: Cell<bool> = const { Cell::new(false) };
}

type SubmitAttacks = unsafe extern "system" fn(*mut c_void, f32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BulletKind {
    Script,
    Normal,
}

#[derive(Debug, Clone, Copy)]
struct BulletReading {
    deals_damage: bool,
    target_group: u32,
    hit_radius: f32,
    beam_end: Option<[f32; 3]>,
    widening: Option<Widening>,
}

#[derive(Debug, Clone, Copy)]
struct Widening {
    radius_growth_rate: f32,
    effects_scale_with_radius: bool,
    ground_effect_id: i32,
}

#[derive(Debug, Clone, Copy)]
struct FirstSight {
    kind: BulletKind,
    enemy: EnemyId,
    target_before: [u8; 8],
    within_reach: bool,
    second_pass: bool,
}

struct TargetSwap {
    address: u64,
    before: [u8; 8],
}

impl Drop for TargetSwap {
    fn drop(&mut self) {
        let Some(build) = HOOKED_BUILD.get() else {
            return;
        };
        if InProcessMemory::new(build)
            .write(self.address, &self.before)
            .is_err()
        {
            log_line(&format!(
                "bullets: could not put back the target of the bullet at {:#x}",
                self.address - BULLET_TARGET_HANDLE_OFFSET
            ));
        }
    }
}

pub(super) fn install(build: VerifiedBuild) {
    if HOOKED_BUILD.set(build).is_err() {
        return;
    }
    #[expect(
        unsafe_code,
        reason = "hooks the two bullet classes' attack submission by address"
    )]
    // SAFETY: both RVAs start a bullet class's vslot 0x760 body (bullet, hit radius in xmm1, no
    // result) in the verified build, called by the bullet manager each frame; the replacements
    // have the same two arguments and pass them on unchanged.
    unsafe {
        hook_address(
            "CScriptBullet::SubmitAttacks",
            (build.module_base() + u64::from(SCRIPT_BULLET_SUBMIT_ATTACKS_FUNCTION_RVA)) as usize,
            script_submit_detour as *const () as *mut c_void,
            &ORIGINAL_SCRIPT_SUBMIT,
        );
        hook_address(
            "CNormalBullet::SubmitAttacks",
            (build.module_base() + u64::from(NORMAL_BULLET_SUBMIT_ATTACKS_FUNCTION_RVA)) as usize,
            normal_submit_detour as *const () as *mut c_void,
            &ORIGINAL_NORMAL_SUBMIT,
        );
    }
}

extern "system" fn script_submit_detour(bullet: *mut c_void, hit_radius: f32) {
    submit_with_puppet_pass(
        &ORIGINAL_SCRIPT_SUBMIT,
        BulletKind::Script,
        bullet,
        hit_radius,
    );
}

extern "system" fn normal_submit_detour(bullet: *mut c_void, hit_radius: f32) {
    submit_with_puppet_pass(
        &ORIGINAL_NORMAL_SUBMIT,
        BulletKind::Normal,
        bullet,
        hit_radius,
    );
}

fn submit_with_puppet_pass(
    original: &AtomicPtr<c_void>,
    kind: BulletKind,
    bullet: *mut c_void,
    hit_radius: f32,
) {
    let original = original.load(Ordering::Acquire);
    if original.is_null() {
        return;
    }
    call_original(original, bullet, hit_radius);
    if IN_SECOND_PASS.with(Cell::get) {
        return;
    }
    let swap = panic::catch_unwind(|| swap_target_to_puppet(kind, bullet as u64))
        .ok()
        .flatten();
    let Some(swap) = swap else {
        return;
    };
    IN_SECOND_PASS.with(|flag| flag.set(true));
    call_original(original, bullet, hit_radius);
    IN_SECOND_PASS.with(|flag| flag.set(false));
    let _ = panic::catch_unwind(move || drop(swap));
}

#[expect(
    unsafe_code,
    reason = "calls the original SubmitAttacks through the trampoline MinHook built"
)]
fn call_original(original: *mut c_void, bullet: *mut c_void, hit_radius: f32) {
    // SAFETY: the trampoline continues into the original body, which has this exact signature,
    // with the bullet and radius the bullet manager passed in on this thread.
    unsafe {
        core::mem::transmute::<*mut c_void, SubmitAttacks>(original)(bullet, hit_radius);
    }
}

fn swap_target_to_puppet(kind: BulletKind, bullet: u64) -> Option<TargetSwap> {
    let build = HOOKED_BUILD.get()?;
    let target = with_view(|view| {
        (view.role == Some(EnemySyncRole::Host))
            .then_some(view.bullet_target)
            .flatten()
    })?;
    let mut memory = InProcessMemory::new(build);
    let reading = read_bullet(&memory, kind, bullet)?;
    if !reading.deals_damage {
        return None;
    }
    let within_reach = bullet_reaches_puppet(
        target.position,
        read_actor_position(&memory, bullet).ok()?,
        reading.beam_end,
        reading.hit_radius,
    );
    if !within_reach && reading.beam_end.is_none() {
        return None;
    }
    let target_address = bullet + BULLET_TARGET_HANDLE_OFFSET;
    let before = memory.read_array::<8>(target_address).ok()?;
    let fired_by = bullet_owner(&memory, bullet)
        .and_then(|owner| with_view(|view| view.enemies.get(&owner).copied()));
    let inputs = PuppetPassInputs {
        puppet_takes_bullets: true,
        in_second_pass: false,
        deals_damage: true,
        aims_at_puppet: before == target.handle,
        within_reach,
        fired_by_enemy: fired_by.is_some(),
    };
    let second_pass = needs_puppet_pass(inputs);
    if let Some(enemy) = fired_by {
        let sight = FirstSight {
            kind,
            enemy,
            target_before: before,
            within_reach,
            second_pass,
        };
        log_first_sight(&memory, bullet, sight, &reading, target);
    }
    if !second_pass {
        return None;
    }
    let swap = TargetSwap {
        address: target_address,
        before,
    };
    memory.write(target_address, &target.handle).ok()?;
    Some(swap)
}

fn read_bullet(memory: &InProcessMemory, kind: BulletKind, bullet: u64) -> Option<BulletReading> {
    let target_group = memory
        .read_i32(bullet + BULLET_TARGET_GROUP_OFFSET)
        .ok()?
        .cast_unsigned();
    let hit_radius = memory
        .read_f32(bullet + BULLET_HIT_SPHERE_RADIUS_OFFSET)
        .ok()?;
    match kind {
        BulletKind::Script => {
            let beam_end = (memory
                .read_u8(bullet + SCRIPT_BULLET_BEAM_TO_SENIOR_FLAG_OFFSET)
                .ok()?
                != 0)
                .then(|| ActorHandle::read(memory, bullet + SCRIPT_BULLET_SENIOR_HANDLE_OFFSET))
                .flatten()
                .and_then(|senior| senior.resolve(memory))
                .and_then(|senior| read_actor_position(memory, senior).ok());
            Some(BulletReading {
                deals_damage: script_bullet_deals_damage(
                    target_group,
                    hit_radius,
                    beam_end.is_some(),
                ),
                target_group,
                hit_radius,
                beam_end,
                widening: None,
            })
        }
        BulletKind::Normal => {
            let settings = memory
                .read_u64(bullet + NORMAL_BULLET_ATTACK_SETTINGS_POINTER_OFFSET)
                .ok()?;
            let record = memory
                .read_u64(bullet + NORMAL_BULLET_RECORD_POINTER_OFFSET)
                .ok()?;
            let widening = (settings != 0).then(|| Widening {
                radius_growth_rate: memory
                    .read_f32(bullet + NORMAL_BULLET_HIT_RADIUS_GROWTH_RATE_OFFSET)
                    .unwrap_or(0.0),
                effects_scale_with_radius: memory
                    .read_f32(settings + NORMAL_BULLET_SETTINGS_EFFECT_SCALE_DIVISOR_OFFSET)
                    .is_ok_and(|divisor| divisor > 0.0),
                ground_effect_id: memory
                    .read_i32(settings + NORMAL_BULLET_SETTINGS_GROUND_EFFECT_ID_OFFSET)
                    .unwrap_or(0),
            });
            Some(BulletReading {
                deals_damage: settings != 0 && record != 0,
                target_group,
                hit_radius,
                beam_end: None,
                widening,
            })
        }
    }
}

fn log_first_sight(
    memory: &InProcessMemory,
    bullet: u64,
    sight: FirstSight,
    reading: &BulletReading,
    puppet: PuppetBulletTarget,
) {
    let FirstSight {
        kind,
        enemy,
        target_before,
        within_reach,
        second_pass,
    } = sight;
    let Ok(own_handle) = memory.read_u64(bullet + ACTOR_SELF_HANDLE_OFFSET) else {
        return;
    };
    if !LOGGED_BULLETS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .first_sight(own_handle)
    {
        return;
    }
    let host_handle = with_view(|view| host_handle(memory, view));
    let aims_at = if target_before == puppet.handle {
        "guest puppet"
    } else if Some(target_before) == host_handle {
        "host body"
    } else if target_before == [0; 8] {
        "none"
    } else {
        "other"
    };
    let widening = reading.widening.map_or_else(String::new, |widening| {
        format!(
            ", width grows {:.2}, effects scale with it {}, ground effect {}",
            widening.radius_growth_rate,
            widening.effects_scale_with_radius,
            widening.ground_effect_id
        )
    });
    log_line(&format!(
        "bullets: {}'s {} bullet {own_handle:#x}: target {aims_at} (hint {:#x}, host {:#x}, puppet {:#x}), target group {:#x}, beam {}, width {:.2}{widening}, in reach {within_reach}, second pass {second_pass}",
        enemy_id_text(enemy),
        match kind {
            BulletKind::Script => "script",
            BulletKind::Normal => "ordinary",
        },
        u64::from_le_bytes(target_before),
        host_handle.map_or(0, u64::from_le_bytes),
        u64::from_le_bytes(puppet.handle),
        reading.target_group,
        reading.beam_end.is_some(),
        reading.hit_radius,
    ));
}

fn host_handle(memory: &InProcessMemory, view: &CombatView) -> Option<[u8; 8]> {
    (view.local_player != 0)
        .then(|| {
            memory
                .read_array::<8>(view.local_player + ACTOR_SELF_HANDLE_OFFSET)
                .ok()
        })
        .flatten()
}
