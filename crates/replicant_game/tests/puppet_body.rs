use replicant_game::puppet::{
    BODY_RESEND_INTERVAL_MS, BODY_TABLE, BodyAnnouncer, BodyPackLoad, BodyPackStep, BookAnnouncer,
    GhostBook, MotionEntry, RespawnGate, RetirePlan, RetryGate, SpawnResult, body_for_pack,
    body_for_seed, body_info, idle_index_from_table, own_seed_swap, release_requested_pack_wanted,
    retire_plan,
};
use replicant_game::puppet::{PACK_REQUEST_RETRY_MS, PACK_SETTLE_AFTER_FOUND_MS};
use replicant_game::stay_together::FollowRole;
use replicant_proto::PartnerBody;

const ALL_BODIES: [PartnerBody; 5] = [
    PartnerBody::Nier,
    PartnerBody::PrologueCharacter,
    PartnerBody::SeedNierY,
    PartnerBody::SeedNierF,
    PartnerBody::SeedKaineE,
];

#[test]
fn every_body_has_exactly_one_table_row_with_a_distinct_pack() {
    for body in ALL_BODIES {
        let rows = BODY_TABLE.iter().filter(|row| row.body == body).count();
        assert_eq!(rows, 1);
    }
    assert_eq!(body_info(PartnerBody::Nier).pack_name, "NIER013");
    assert_eq!(
        body_info(PartnerBody::PrologueCharacter).pack_name,
        "NIER014"
    );
}

#[test]
fn every_protagonist_body_tries_the_shared_idle_motion_first_and_shows_the_weapon() {
    for row in BODY_TABLE {
        if row.body != PartnerBody::SeedKaineE {
            assert_eq!(row.idle_motions[0], "NIER013_idle_lp");
        }
        assert!(row.weapon_shown);
        assert_eq!((row.request_kind, row.request_slot_hint), (0, 1));
    }
}

#[test]
fn a_resident_pack_is_spawned_at_once_and_never_released() {
    let mut load = BodyPackLoad::new();
    assert_eq!(load.step(true, 0), BodyPackStep::Spawn);
    assert!(!load.requested_by_us());
}

#[test]
fn a_missing_pack_is_requested_once_then_awaited_then_spawned_after_it_settles() {
    let mut load = BodyPackLoad::new();
    assert_eq!(load.step(false, 0), BodyPackStep::Request);
    assert_eq!(load.step(false, 500), BodyPackStep::Wait);
    assert_eq!(load.step(true, 900), BodyPackStep::Wait);
    assert_eq!(
        load.step(true, 900 + PACK_SETTLE_AFTER_FOUND_MS),
        BodyPackStep::Spawn
    );
    assert!(load.requested_by_us());
}

#[test]
fn a_pack_that_never_shows_up_is_requested_again_after_the_retry_time() {
    let mut load = BodyPackLoad::new();
    assert_eq!(load.step(false, 0), BodyPackStep::Request);
    assert_eq!(
        load.step(false, PACK_REQUEST_RETRY_MS),
        BodyPackStep::Request
    );
}

#[test]
fn only_a_pack_we_requested_and_the_local_player_does_not_wear_is_released() {
    assert!(release_requested_pack_wanted(true, false));
    assert!(!release_requested_pack_wanted(true, true));
    assert!(!release_requested_pack_wanted(false, false));
}

#[test]
fn the_body_is_announced_on_change_and_then_once_per_interval() {
    let mut announcer = BodyAnnouncer::new();
    assert!(announcer.is_due(PartnerBody::Nier, 1, 0));
    assert!(!announcer.is_due(PartnerBody::Nier, 1, 10));
    assert!(announcer.is_due(PartnerBody::PrologueCharacter, 1, 20));
    assert!(!announcer.is_due(PartnerBody::PrologueCharacter, 1, 30));
    assert!(announcer.is_due(
        PartnerBody::PrologueCharacter,
        1,
        20 + BODY_RESEND_INTERVAL_MS
    ));
}

#[test]
fn every_phase_load_announces_the_body_again_even_when_it_did_not_change() {
    let mut announcer = BodyAnnouncer::new();
    assert!(announcer.is_due(PartnerBody::Nier, 1, 0));
    assert!(!announcer.is_due(PartnerBody::Nier, 1, 10));
    assert!(announcer.is_due(PartnerBody::Nier, 2, 20));
}

#[test]
fn a_pack_name_and_a_seed_name_find_the_body_they_belong_to() {
    assert_eq!(body_for_pack("NIER013"), Some(PartnerBody::Nier));
    assert_eq!(
        body_for_pack("NIER014"),
        Some(PartnerBody::PrologueCharacter)
    );
    assert_eq!(body_for_pack("NIER999"), None);
    assert_eq!(body_for_seed("nierB"), Some(PartnerBody::Nier));
    assert_eq!(body_for_seed("nierT"), Some(PartnerBody::PrologueCharacter));
    assert_eq!(body_for_seed("nierY"), Some(PartnerBody::SeedNierY));
    assert_eq!(body_for_seed("nierF"), Some(PartnerBody::SeedNierF));
    assert_eq!(body_for_seed("kaineE"), Some(PartnerBody::SeedKaineE));
    assert_eq!(body_for_seed("nier010"), None);
}

#[test]
fn every_seed_in_the_game_seed_table_maps_to_a_distinct_body_pack() {
    let seed_table = ["nierB", "nierT", "nierY", "nierF", "kaineE"];
    let packs = ["NIER013", "NIER014", "NIER025", "NIER030", "KAINE015"];
    for (index, (seed, pack)) in seed_table.iter().zip(packs).enumerate() {
        let body = body_for_seed(seed).expect("seed has a body");
        let info = body_info(body);
        assert_eq!(info.pack_name, pack);
        assert_eq!(info.own_seed_name, *seed);
        assert_eq!(info.own_seed_table_index as usize, index);
        assert_eq!(body_for_pack(pack), Some(body));
    }
    assert_eq!(BODY_TABLE.len(), seed_table.len());
}

#[test]
fn the_host_and_an_unknown_role_never_swap_their_own_body() {
    for role in [FollowRole::Host, FollowRole::Unknown] {
        assert_eq!(own_seed_swap(role, "nierB"), None);
    }
}

#[test]
fn a_guest_gets_the_swapped_body_in_place_of_every_protagonist_seed() {
    for seed in ["nierB", "nierT", "nierY", "nierF"] {
        let swap = own_seed_swap(FollowRole::Guest, seed).expect("swap");
        assert_eq!((swap.from, swap.to, swap.table_index), (seed, "kaineE", 4));
    }
}

#[test]
fn a_guest_keeps_the_swapped_seed_and_unknown_names_as_they_are() {
    for seed in ["kaineE", "nier010", ""] {
        assert_eq!(own_seed_swap(FollowRole::Guest, seed), None);
    }
}

#[test]
fn the_seed_table_indexes_match_the_games_name_order() {
    assert_eq!(body_info(PartnerBody::Nier).own_seed_table_index, 0);
    assert_eq!(
        body_info(PartnerBody::PrologueCharacter).own_seed_table_index,
        1
    );
}

fn entry(index: i32, name: &str) -> MotionEntry {
    MotionEntry {
        index,
        name: name.to_owned(),
    }
}

#[test]
fn the_kaine_body_idles_on_her_own_pack_and_shows_her_forced_weapon() {
    let kaine = body_info(PartnerBody::SeedKaineE);
    assert!(
        kaine
            .idle_motions
            .iter()
            .all(|name| name.starts_with("KAINE015_"))
    );
    assert!(kaine.weapon_shown);
}

#[test]
fn the_prologue_body_has_a_second_idle_name_to_try() {
    assert!(body_info(PartnerBody::PrologueCharacter).idle_motions.len() >= 2);
}

#[test]
fn a_missing_idle_name_falls_back_to_any_idle_loop_in_the_table() {
    let table = [
        entry(3, "walk_lp"),
        entry(9, "NIER014_idle_lp"),
        entry(10, "idle_lp2"),
    ];
    assert_eq!(idle_index_from_table(&table), Some(9));
    assert_eq!(idle_index_from_table(&[entry(3, "walk_lp")]), None);
    assert_eq!(idle_index_from_table(&[]), None);
}

#[test]
fn an_empty_motion_table_is_only_read_again_after_the_retry_interval() {
    let mut gate = RetryGate::new(1000);
    assert!(gate.is_due(0));
    assert!(!gate.is_due(10));
    assert!(!gate.is_due(999));
    assert!(gate.is_due(1000));
    assert!(!gate.is_due(1500));
}

#[test]
fn every_retire_plan_releases_the_requested_pack() {
    use RetirePlan::*;
    assert_eq!(retire_plan(true, true), ReleasePackWhichRemovesActor);
    assert_eq!(retire_plan(true, false), DespawnThenReleasePack);
    assert_eq!(retire_plan(false, true), ForgetActorThenReleasePack);
    assert_eq!(retire_plan(false, false), ForgetActorThenReleasePack);
}

#[test]
fn an_adopted_live_puppet_keeps_its_real_body() {
    let mut book = GhostBook::new();
    book.note_spawn(
        PartnerBody::PrologueCharacter,
        SpawnResult::AlreadyAlive {
            id: 4,
            body: PartnerBody::Nier,
        },
    );
    assert_eq!(book.puppet_id, Some(4));
    assert_eq!(book.puppet_body, Some(PartnerBody::Nier));
    book.note_spawn(PartnerBody::PrologueCharacter, SpawnResult::Created(5));
    assert_eq!(book.puppet_body, Some(PartnerBody::PrologueCharacter));
}

#[test]
fn a_failed_despawn_on_respawn_request_keeps_the_id_and_resets_the_pack_state() {
    let mut book = GhostBook::new();
    book.note_spawn(PartnerBody::Nier, SpawnResult::Created(2));
    book.pack_body = Some(PartnerBody::Nier);
    book.pack_load.step(false, 0);
    book.note_respawn_request(true);
    assert_eq!(book.puppet_id, Some(2));
    assert_eq!(book.puppet_body, None);
    assert_eq!(book.pack_body, None);
    assert!(!book.pack_load.requested_by_us());
    book.note_respawn_request(false);
    assert_eq!(book.puppet_id, None);
}

#[test]
fn a_released_model_is_respawned_only_with_its_pack_loaded_and_two_seconds_apart() {
    let mut gate = RespawnGate::default();
    assert!(!gate.may_spawn(false, 0));
    assert!(gate.may_spawn(true, 10));
    assert!(!gate.may_spawn(true, 20));
    assert!(!gate.may_spawn(false, 2010));
    assert!(!gate.may_spawn(true, 2009));
    assert!(gate.may_spawn(true, 2010));
}

#[test]
fn the_book_presence_is_announced_on_change_and_then_once_per_interval() {
    let mut announcer = BookAnnouncer::new();
    assert!(announcer.is_due(false, 0));
    assert!(!announcer.is_due(false, 10));
    assert!(announcer.is_due(true, 20));
    assert!(!announcer.is_due(true, 30));
    assert!(announcer.is_due(true, 20 + BODY_RESEND_INTERVAL_MS));
}
