use std::collections::HashMap;

use replicant_game::enemies::credited_enemy;
use replicant_game::enemy_bullets::{
    BulletFirstSightings, MAX_BULLETS_REMEMBERED_FOR_LOG, PuppetPassInputs, bullet_reaches_puppet,
    needs_puppet_pass, script_bullet_deals_damage,
};
use replicant_proto::EnemyId;

const ENEMY_ACTOR: u64 = 0x1000;
const BULLET_ACTOR: u64 = 0x2000;
const HOST_ACTOR: u64 = 0x3000;

fn ready_bullet() -> PuppetPassInputs {
    PuppetPassInputs {
        puppet_takes_bullets: true,
        in_second_pass: false,
        deals_damage: true,
        aims_at_puppet: false,
        within_reach: true,
        fired_by_enemy: true,
    }
}

fn enemies() -> HashMap<u64, EnemyId> {
    HashMap::from([(
        ENEMY_ACTOR,
        EnemyId {
            group: 2,
            ordinal: 0,
        },
    )])
}

#[test]
fn a_bullet_is_tested_against_the_guests_copy_once_a_frame() {
    assert!(needs_puppet_pass(ready_bullet()));
    assert!(!needs_puppet_pass(PuppetPassInputs {
        in_second_pass: true,
        ..ready_bullet()
    }));
    assert!(!needs_puppet_pass(PuppetPassInputs {
        aims_at_puppet: true,
        ..ready_bullet()
    }));
}

#[test]
fn no_second_pass_without_a_partner_in_the_phase() {
    assert!(!needs_puppet_pass(PuppetPassInputs {
        puppet_takes_bullets: false,
        ..ready_bullet()
    }));
}

#[test]
fn no_second_pass_for_a_bullet_the_players_fired_or_one_that_deals_nothing() {
    assert!(!needs_puppet_pass(PuppetPassInputs {
        fired_by_enemy: false,
        ..ready_bullet()
    }));
    assert!(!needs_puppet_pass(PuppetPassInputs {
        deals_damage: false,
        ..ready_bullet()
    }));
    assert!(!needs_puppet_pass(PuppetPassInputs {
        within_reach: false,
        ..ready_bullet()
    }));
}

#[test]
fn a_bullets_hit_is_credited_to_the_enemy_that_fired_it() {
    let enemies = enemies();
    let enemy_of = |actor: u64| enemies.get(&actor).copied();
    let fired_it = enemies.get(&ENEMY_ACTOR).map(|id| (ENEMY_ACTOR, *id));
    assert_eq!(
        credited_enemy([Some(BULLET_ACTOR), None], Some(ENEMY_ACTOR), enemy_of),
        fired_it
    );
    assert_eq!(
        credited_enemy([Some(ENEMY_ACTOR), None], None, enemy_of),
        fired_it
    );
    assert_eq!(
        credited_enemy([Some(BULLET_ACTOR), None], Some(HOST_ACTOR), enemy_of),
        None
    );
}

#[test]
fn a_beam_reaches_a_copy_beside_its_middle_but_not_one_past_its_end() {
    let start = [0.0, 1.0, 0.0];
    let end = [66.0, 1.0, 0.0];
    assert!(bullet_reaches_puppet(
        [33.0, 0.0, 1.0],
        start,
        Some(end),
        0.5
    ));
    assert!(!bullet_reaches_puppet([33.0, 0.0, 1.0], start, None, 0.5));
    assert!(!bullet_reaches_puppet(
        [80.0, 1.0, 0.0],
        start,
        Some(end),
        0.5
    ));
}

#[test]
fn a_script_bullet_without_a_target_group_deals_nothing() {
    assert!(!script_bullet_deals_damage(0, 1.0, true));
    assert!(script_bullet_deals_damage(6, 0.0, true));
    assert!(!script_bullet_deals_damage(6, 0.0, false));
}

#[test]
fn each_bullet_is_logged_once() {
    let mut sightings = BulletFirstSightings::default();
    assert!(sightings.first_sight(7));
    assert!(!sightings.first_sight(7));
    for bullet in 100..100 + MAX_BULLETS_REMEMBERED_FOR_LOG as u64 {
        sightings.first_sight(bullet);
    }
    assert!(sightings.first_sight(7));
}
