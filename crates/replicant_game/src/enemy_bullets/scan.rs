use std::collections::HashMap;

use replicant_proto::{BulletSprite, EnemyId};

use crate::bullet_flights::{BULLET_STATE_RUNNING, SightedBullet, read_bullet};
use crate::enemies::ActorHandle;
use crate::enemy_bullets::{
    LaserBeam, MAX_SIGHTED_ENEMY_BULLETS, MAX_SIGHTED_ENEMY_SPRITE_BULLETS,
    MAX_UNCOPIED_SCRIPT_BULLETS_READ, NotCopyable, UncopiedWhy, read_laser_beam,
    read_script_bullet, script_bullet_is_laser, shot_scale_growth, why_not_copied,
};
use crate::game::GameError;
use crate::memory::GameMemory;
use crate::offsets::{
    HANDLE_POOL_POINTERS_RVA, HANDLE_POOL_SLOT_COUNT, NORMAL_BULLET_OWNER_HANDLE_OFFSET,
    NORMAL_BULLET_STATE_OFFSET, NORMAL_BULLET_VTABLE_RVA, SCRIPT_BULLET_VTABLE_RVA,
    TRACE_BULLET_VTABLE_RVA,
};
use crate::puppet::actor_is_live;

const POINTER_BYTES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnemyBullet {
    pub enemy: EnemyId,
    pub bullet: SightedBullet,
    /// Set for a bullet the game draws through its sprite batch; its look is then empty.
    pub sprite: Option<BulletSprite>,
    /// Set for a script bullet whose effect the game stretches to another bullet each frame.
    pub beam: Option<LaserBeam>,
    /// See `EnemyBulletFlight::scale_growth_per_second`.
    pub scale_growth_per_second: f32,
}

/// The running bullets of shared enemies in one scan, by what draws them. `normal`,
/// `script_with_effect` and `script_sprite` bullets can be shown to the guest.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EnemyBulletCensus {
    pub normal: u32,
    pub normal_not_drawn: u32,
    pub script_with_effect: u32,
    pub script_sprite: u32,
    pub script_sprite_not_drawn: u32,
    pub script_without_effect: u32,
    pub trace: u32,
    pub past_the_limit: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnemyBulletScan {
    pub bullets: Vec<EnemyBullet>,
    pub census: EnemyBulletCensus,
    pub uncopied: Vec<UncopiedScriptBullet>,
}

/// A running script bullet of a shared enemy that the scan counted as having no effect to copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UncopiedScriptBullet {
    pub enemy: EnemyId,
    pub why: UncopiedWhy,
    pub laser: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BulletClass {
    Normal,
    Script,
    Trace,
}

impl EnemyBulletCensus {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    pub fn keep_highest(&mut self, other: &Self) {
        self.normal = self.normal.max(other.normal);
        self.normal_not_drawn = self.normal_not_drawn.max(other.normal_not_drawn);
        self.script_with_effect = self.script_with_effect.max(other.script_with_effect);
        self.script_sprite = self.script_sprite.max(other.script_sprite);
        self.script_sprite_not_drawn = self
            .script_sprite_not_drawn
            .max(other.script_sprite_not_drawn);
        self.script_without_effect = self.script_without_effect.max(other.script_without_effect);
        self.trace = self.trace.max(other.trace);
        self.past_the_limit = self.past_the_limit.max(other.past_the_limit);
    }
}

/// Every running, drawn bullet in the handle pool whose owner is one of `enemies` (actor address
/// to shared enemy id): at most `MAX_SIGHTED_ENEMY_BULLETS` drawn by effects and
/// `MAX_SIGHTED_ENEMY_SPRITE_BULLETS` drawn as sprites.
pub fn bullets_of_enemies<M: GameMemory>(
    memory: &M,
    enemies: &HashMap<u64, EnemyId>,
) -> Result<Vec<EnemyBullet>, GameError> {
    scan_enemy_bullets(memory, enemies).map(|scan| scan.bullets)
}

/// As `bullets_of_enemies`, with a count of every bullet class the enemies have in the air.
pub fn scan_enemy_bullets<M: GameMemory>(
    memory: &M,
    enemies: &HashMap<u64, EnemyId>,
) -> Result<EnemyBulletScan, GameError> {
    let mut scan = EnemyBulletScan::default();
    if enemies.is_empty() {
        return Ok(scan);
    }
    let mut table = vec![0_u8; HANDLE_POOL_SLOT_COUNT as usize * POINTER_BYTES];
    memory.read(
        memory.module_base() + u64::from(HANDLE_POOL_POINTERS_RVA),
        &mut table,
    )?;
    let (pointers, _) = table.as_chunks::<POINTER_BYTES>();
    let mut effect_bullets_sighted = 0_usize;
    let mut sprites_sighted = 0_usize;
    for (slot, bytes) in (0_u16..).zip(pointers) {
        let actor = u64::from_le_bytes(*bytes);
        if actor == 0 {
            continue;
        }
        let Some(class) = bullet_class(memory, actor) else {
            continue;
        };
        let Some(enemy) = ActorHandle::read(memory, actor + NORMAL_BULLET_OWNER_HANDLE_OFFSET)
            .and_then(|handle| handle.resolve(memory))
            .and_then(|owner| enemies.get(&owner).copied())
        else {
            continue;
        };
        if !actor_is_live(memory, actor).unwrap_or(false)
            || memory.read_i32(actor + NORMAL_BULLET_STATE_OFFSET) != Ok(BULLET_STATE_RUNNING)
        {
            continue;
        }
        let mut no_effect = false;
        let drawn = drawn_bullet(memory, actor, slot, class, &mut scan.census, &mut no_effect);
        let Some((bullet, sprite)) = drawn else {
            if no_effect && scan.uncopied.len() < MAX_UNCOPIED_SCRIPT_BULLETS_READ {
                scan.uncopied.push(UncopiedScriptBullet {
                    enemy,
                    why: why_not_copied(memory, actor),
                    laser: script_bullet_is_laser(memory, actor),
                });
            }
            continue;
        };
        let (sighted, limit) = match sprite {
            Some(_) => (&mut sprites_sighted, MAX_SIGHTED_ENEMY_SPRITE_BULLETS),
            None => (&mut effect_bullets_sighted, MAX_SIGHTED_ENEMY_BULLETS),
        };
        if *sighted >= limit {
            scan.census.past_the_limit += 1;
            continue;
        }
        *sighted += 1;
        let beam = match (class, sprite) {
            (BulletClass::Script, None) => read_laser_beam(memory, actor),
            _ => None,
        };
        let scale_growth_per_second = match class {
            BulletClass::Normal => shot_scale_growth(memory, actor),
            BulletClass::Script | BulletClass::Trace => 0.0,
        };
        scan.bullets.push(EnemyBullet {
            enemy,
            bullet,
            sprite,
            beam,
            scale_growth_per_second,
        });
    }
    Ok(scan)
}

pub(super) fn bullet_class<M: GameMemory>(memory: &M, actor: u64) -> Option<BulletClass> {
    let class = memory.read_u64(actor).ok()?;
    let base = memory.module_base();
    if class == base + u64::from(NORMAL_BULLET_VTABLE_RVA) {
        Some(BulletClass::Normal)
    } else if class == base + u64::from(SCRIPT_BULLET_VTABLE_RVA) {
        Some(BulletClass::Script)
    } else if class == base + u64::from(TRACE_BULLET_VTABLE_RVA) {
        Some(BulletClass::Trace)
    } else {
        None
    }
}

fn drawn_bullet<M: GameMemory>(
    memory: &M,
    actor: u64,
    slot: u16,
    class: BulletClass,
    census: &mut EnemyBulletCensus,
    no_effect: &mut bool,
) -> Option<(SightedBullet, Option<BulletSprite>)> {
    match class {
        BulletClass::Normal => {
            let bullet = read_bullet(memory, actor, slot);
            match bullet {
                Some(_) => census.normal += 1,
                None => census.normal_not_drawn += 1,
            }
            bullet.map(|bullet| (bullet, None))
        }
        BulletClass::Script => match read_script_bullet(memory, actor, slot) {
            Ok((bullet, sprite)) => {
                match sprite {
                    Some(_) => census.script_sprite += 1,
                    None => census.script_with_effect += 1,
                }
                Some((bullet, sprite))
            }
            Err(NotCopyable::SpriteNotDrawn) => {
                census.script_sprite_not_drawn += 1;
                None
            }
            Err(NotCopyable::NoEffect) => {
                census.script_without_effect += 1;
                *no_effect = true;
                None
            }
        },
        BulletClass::Trace => {
            census.trace += 1;
            None
        }
    }
}
