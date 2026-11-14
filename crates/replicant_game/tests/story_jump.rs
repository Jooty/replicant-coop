use replicant_game::PhaseEntry;
use replicant_game::story_jump::{
    JumpStep, LEVEL_CHECK_SETTLE_MS, LevelCheck, LevelWatcher, StoryJumpRefusal, StoryJumpView,
    arrival_is_due, arrival_spot, check_level, check_story_jump, destination,
    jump_waits_for_pause_menu, level_check_is_due, loadout_player_level, loadout_steps,
};
use replicant_proto::StoryJumpPoint;

fn ready_host() -> StoryJumpView {
    StoryJumpView {
        host: true,
        map_manager_active: true,
        event_scene: false,
        talk_window_open: false,
        ui_dialog_state: 0,
        pause_menu_open: false,
    }
}

#[test]
fn a_host_in_the_field_may_jump() {
    assert_eq!(check_story_jump(&ready_host()), Ok(()));
}

#[test]
fn a_guest_is_refused() {
    let view = StoryJumpView {
        host: false,
        ..ready_host()
    };
    assert_eq!(check_story_jump(&view), Err(StoryJumpRefusal::NotHost));
}

#[test]
fn a_host_outside_the_field_is_refused() {
    let loading = StoryJumpView {
        map_manager_active: false,
        ..ready_host()
    };
    assert_eq!(
        check_story_jump(&loading),
        Err(StoryJumpRefusal::NotInTheField)
    );
}

#[test]
fn an_event_scene_refuses_the_jump() {
    let scene = StoryJumpView {
        event_scene: true,
        ..ready_host()
    };
    assert_eq!(check_story_jump(&scene), Err(StoryJumpRefusal::EventScene));
}

#[test]
fn an_open_talk_refuses_the_jump() {
    let talk = StoryJumpView {
        talk_window_open: true,
        ..ready_host()
    };
    assert_eq!(check_story_jump(&talk), Err(StoryJumpRefusal::TalkOpen));
}

#[test]
fn an_open_dialog_refuses_the_jump_and_names_its_state() {
    let dialog = StoryJumpView {
        ui_dialog_state: 4,
        ..ready_host()
    };
    let refusal = check_story_jump(&dialog).unwrap_err();
    assert_eq!(refusal, StoryJumpRefusal::DialogOpen(4));
    assert!(refusal.to_string().contains("0x4"));
}

#[test]
fn the_pause_menu_does_not_refuse_the_jump_but_makes_it_wait() {
    let paused = StoryJumpView {
        pause_menu_open: true,
        ..ready_host()
    };
    assert_eq!(check_story_jump(&paused), Ok(()));
    assert!(jump_waits_for_pause_menu(&paused));
    assert!(!jump_waits_for_pause_menu(&ready_host()));
}

#[test]
fn after_the_prologue_sets_the_end_flags_and_the_loadout() {
    let steps = loadout_steps(StoryJumpPoint::AfterPrologue);
    for expected in [
        JumpStep::SetFlag { id: 71 },
        JumpStep::SetFlag { id: 153 },
        JumpStep::SetItem { id: 1000, count: 1 },
        JumpStep::EquipWeapon { id: 1000 },
        JumpStep::SetPlayerLevel { level: 0 },
        JumpStep::MpFullMode { on: false },
    ] {
        assert!(steps.contains(&expected), "{expected:?}");
    }
}

#[test]
fn after_the_prologue_clears_the_resume_flags() {
    let steps = loadout_steps(StoryJumpPoint::AfterPrologue);
    assert!(steps.contains(&JumpStep::ClearFlag { id: 1403 }));
    assert!(steps.contains(&JumpStep::ClearFlag { id: 149 }));
}

#[test]
fn after_the_prologue_lands_on_a_known_phase_and_a_valid_point() {
    let (name, point) = destination(StoryJumpPoint::AfterPrologue);
    assert!(PhaseEntry::find(name).is_some());
    assert!((0..=replicant_game::offsets::MAX_PHASE_POINT).contains(&point));
}

#[test]
fn the_popola_talk_adds_its_flags_and_keeps_the_loadout() {
    let steps = loadout_steps(StoryJumpPoint::AtPopolaTalk);
    for expected in [
        JumpStep::SetFlag { id: 71 },
        JumpStep::SetFlag { id: 153 },
        JumpStep::SetFlag { id: 154 },
        JumpStep::SetFlag { id: 1299 },
        JumpStep::SetFlag { id: 1401 },
        JumpStep::ClearFlag { id: 1403 },
        JumpStep::ClearFlag { id: 149 },
        JumpStep::EquipWeapon { id: 1000 },
        JumpStep::MpFullMode { on: false },
    ] {
        assert!(steps.contains(&expected), "{expected:?}");
    }
    assert!(!steps.contains(&JumpStep::SetFlag { id: 156 }));
}

#[test]
fn the_popola_talk_lands_in_the_library_on_a_valid_point() {
    let (name, point) = destination(StoryJumpPoint::AtPopolaTalk);
    assert_eq!(name, "A_CENTER_LIBRARY_01");
    assert!(PhaseEntry::find(name).is_some());
    assert!((0..=replicant_game::offsets::MAX_PHASE_POINT).contains(&point));
}

#[test]
fn before_the_next_day_carries_the_popola_day_flags_and_the_map() {
    let steps = loadout_steps(StoryJumpPoint::BeforeNextDay);
    for expected in [
        JumpStep::SetFlag { id: 71 },
        JumpStep::SetFlag { id: 154 },
        JumpStep::SetFlag { id: 156 },
        JumpStep::SetFlag { id: 163 },
        JumpStep::SetFlag { id: 164 },
        JumpStep::SetFlag { id: 166 },
        JumpStep::SetFlag { id: 1300 },
        JumpStep::ClearFlag { id: 1299 },
        JumpStep::ClearFlag { id: 1401 },
        JumpStep::ClearFlag { id: 1403 },
        JumpStep::SetItem { id: 723, count: 1 },
        JumpStep::EquipWeapon { id: 1000 },
    ] {
        assert!(steps.contains(&expected), "{expected:?}");
    }
    assert!(!steps.contains(&JumpStep::SetFlag { id: 1299 }));
    assert!(!steps.contains(&JumpStep::SetFlag { id: 1401 }));
}

#[test]
fn before_the_next_day_lands_in_the_village_outside_the_library() {
    let (name, point) = destination(StoryJumpPoint::BeforeNextDay);
    assert_eq!(name, "A_CENTER_VILLAGE_01");
    assert_eq!(point, 4);
    assert!(PhaseEntry::find(name).is_some());
    assert!((0..=replicant_game::offsets::MAX_PHASE_POINT).contains(&point));
}

#[test]
fn the_next_day_jump_gives_the_map_item_and_its_tutorial() {
    let steps = loadout_steps(StoryJumpPoint::BeforeNextDay);
    assert!(steps.contains(&JumpStep::SetItem { id: 723, count: 1 }));
    assert!(steps.contains(&JumpStep::SetTutorial { id: 2010 }));
}

#[test]
fn the_jumps_before_the_map_do_not_give_it() {
    for point in [StoryJumpPoint::AfterPrologue, StoryJumpPoint::AtPopolaTalk] {
        let steps = loadout_steps(point);
        assert!(!steps.contains(&JumpStep::SetItem { id: 723, count: 1 }));
        assert!(!steps.contains(&JumpStep::SetTutorial { id: 2010 }));
    }
}

#[test]
fn the_shrine_entrance_jump_lands_in_the_shrine_with_the_map_and_counters() {
    let steps = loadout_steps(StoryJumpPoint::AtShrineEntrance);
    for expected in [
        JumpStep::SetItem { id: 723, count: 1 },
        JumpStep::SetTutorial { id: 2010 },
        JumpStep::SetFlag { id: 167 },
        JumpStep::SetFlag { id: 171 },
        JumpStep::SetFlag { id: 77 },
        JumpStep::SetFlag { id: 1303 },
        JumpStep::SetSnowFlag { id: 1052 },
        JumpStep::SetSnowFlag { id: 1069 },
        JumpStep::ClearFlag { id: 1300 },
        JumpStep::ClearFlag { id: 1443 },
        JumpStep::SetGameCounter {
            id: 100,
            value: 255,
        },
        JumpStep::SetGameCounter {
            id: 101,
            value: 255,
        },
    ] {
        assert!(steps.contains(&expected), "{expected:?}");
    }
    for set in [190, 191] {
        assert!(steps.contains(&JumpStep::SetFlag { id: set }), "{set}");
    }
    for id in 1401..=1410 {
        assert!(steps.contains(&JumpStep::ClearFlag { id }), "{id}");
    }
    for unset in [75, 192, 202, 205] {
        assert!(!steps.contains(&JumpStep::SetFlag { id: unset }), "{unset}");
    }
    let (name, point) = destination(StoryJumpPoint::AtShrineEntrance);
    assert_eq!(name, "A_STONE_TEMPLE_01");
    assert_eq!(point, 1);
    assert!(PhaseEntry::find(name).is_some());
}

#[test]
fn the_shrine_top_jump_lands_in_the_stone_field_with_the_shrine_done() {
    let steps = loadout_steps(StoryJumpPoint::AtShrineTop);
    for expected in [
        JumpStep::SetFlag { id: 190 },
        JumpStep::SetFlag { id: 191 },
        JumpStep::SetFlag { id: 75 },
        JumpStep::SetFlag { id: 192 },
        JumpStep::SetFlag { id: 138 },
        JumpStep::SetFlag { id: 193 },
        JumpStep::SetFlag { id: 1432 },
        JumpStep::SetFlag { id: 205 },
        JumpStep::SetItem { id: 305, count: 0 },
        JumpStep::SetItem { id: 723, count: 1 },
    ] {
        assert!(steps.contains(&expected), "{expected:?}");
    }
    for id in 1401..=1410 {
        assert!(steps.contains(&JumpStep::ClearFlag { id }), "{id}");
        assert!(!steps.contains(&JumpStep::SetFlag { id }), "{id}");
    }
    assert!(!steps.contains(&JumpStep::SetFlag { id: 194 }));
    assert!(!steps.contains(&JumpStep::SetSnowFlag { id: 1053 }));
    let (name, point) = destination(StoryJumpPoint::AtShrineTop);
    assert_eq!(name, "A_STONE_FIELD_01");
    assert_eq!(point, 3);
    assert!(PhaseEntry::find(name).is_some());
}

#[test]
fn the_shrine_boss_jump_lands_in_the_arena_with_the_boss_scene_still_to_play() {
    let top = loadout_steps(StoryJumpPoint::AtShrineTop);
    let steps = loadout_steps(StoryJumpPoint::AtShrineBoss);
    for step in &top {
        assert!(steps.contains(step), "{step:?}");
    }
    for expected in [
        JumpStep::SetFlag { id: 194 },
        JumpStep::SetFlag { id: 206 },
        JumpStep::SetSnowFlag { id: 1053 },
    ] {
        assert!(steps.contains(&expected), "{expected:?}");
    }
    for id in 1401..=1410 {
        assert!(steps.contains(&JumpStep::ClearFlag { id }), "{id}");
        assert!(!steps.contains(&JumpStep::SetFlag { id }), "{id}");
    }
    for id in [127, 1432, 1433, 1434] {
        assert!(steps.contains(&JumpStep::SetFlag { id }), "{id}");
    }
    assert!(steps.contains(&JumpStep::ClearFlag { id: 1303 }));
    let last_1303 = steps.iter().rposition(|step| {
        matches!(
            step,
            JumpStep::SetFlag { id: 1303 } | JumpStep::ClearFlag { id: 1303 }
        )
    });
    assert_eq!(
        last_1303.map(|index| steps[index]),
        Some(JumpStep::ClearFlag { id: 1303 })
    );
    for id in [195, 196, 204, 1435] {
        assert!(!steps.contains(&JumpStep::SetFlag { id }), "{id}");
    }
    for expected in [
        JumpStep::SetHaveWhiteBook { on: true },
        JumpStep::LearnMagic { id: 3 },
        JumpStep::SetMagicSlot { slot: 1, magic: 3 },
        JumpStep::SetTutorial { id: 2002 },
    ] {
        assert!(steps.contains(&expected), "{expected:?}");
    }
    let (name, point) = destination(StoryJumpPoint::AtShrineBoss);
    assert_eq!(name, "A_STONE_FIELD_02");
    assert_eq!(point, 1);
    assert!(PhaseEntry::find(name).is_some());
}

#[test]
fn every_jump_point_expects_level_zero() {
    for point in [
        StoryJumpPoint::AfterPrologue,
        StoryJumpPoint::AtPopolaTalk,
        StoryJumpPoint::BeforeNextDay,
        StoryJumpPoint::AtShrineEntrance,
        StoryJumpPoint::AtShrineTop,
        StoryJumpPoint::AtShrineBoss,
        StoryJumpPoint::BossFight3,
        StoryJumpPoint::JunkHeapFloorB2,
        StoryJumpPoint::JunkHeapBossFight5,
        StoryJumpPoint::AfterBossFight6,
        StoryJumpPoint::BossFight6,
    ] {
        assert_eq!(loadout_player_level(point), Some(0));
    }
}

#[test]
fn a_level_that_did_not_take_is_applied_again() {
    assert_eq!(check_level(0, Some(0)), LevelCheck::Matches);
    assert_eq!(
        check_level(0, Some(21)),
        LevelCheck::ApplyAgain {
            expected: 0,
            actual: 21
        }
    );
    assert_eq!(check_level(0, None), LevelCheck::Unreadable);
}

#[test]
fn the_watcher_reports_only_changes() {
    let mut watcher = LevelWatcher::new();
    assert_eq!(watcher.observe(21), Some((None, 21)));
    assert_eq!(watcher.observe(21), None);
    assert_eq!(watcher.observe(0), Some((Some(21), 0)));
}

#[test]
fn the_level_check_waits_for_the_new_phase_to_settle() {
    let settled = LEVEL_CHECK_SETTLE_MS;
    assert!(!level_check_is_due("A", "A", true, settled));
    assert!(!level_check_is_due("A", "B", false, settled));
    assert!(!level_check_is_due("A", "B", true, settled - 1));
    assert!(level_check_is_due("A", "B", true, settled));
}

#[test]
fn only_the_arena_floor_b2_and_after_boss_fight_6_jumps_place_the_players() {
    for point in [
        StoryJumpPoint::AfterPrologue,
        StoryJumpPoint::AtPopolaTalk,
        StoryJumpPoint::BeforeNextDay,
        StoryJumpPoint::AtShrineEntrance,
        StoryJumpPoint::AtShrineTop,
        StoryJumpPoint::BossFight3,
        StoryJumpPoint::JunkHeapBossFight5,
        StoryJumpPoint::BossFight6,
    ] {
        assert_eq!(arrival_spot(point, true), None);
        assert_eq!(arrival_spot(point, false), None);
    }
    let host = arrival_spot(StoryJumpPoint::AtShrineBoss, true).expect("host spot");
    let guest = arrival_spot(StoryJumpPoint::AtShrineBoss, false).expect("guest spot");
    assert!((host.x + 2.2).abs() < 0.001 && (host.y - 146.8).abs() < 0.001);
    assert!((host.z + 8.6).abs() < 0.001);
    assert!((guest.x - host.x - 2.0).abs() < 0.001);
    assert_eq!((guest.y, guest.z), (host.y, host.z));
}

#[test]
fn the_floor_b2_jump_places_the_host_where_the_lift_lands_and_the_guest_beside_it() {
    let host = arrival_spot(StoryJumpPoint::JunkHeapFloorB2, true).expect("host spot");
    let guest = arrival_spot(StoryJumpPoint::JunkHeapFloorB2, false).expect("guest spot");
    assert_eq!((host.x, host.y, host.z), (-76.5, 4.9, -124.0));
    assert!((guest.x + 77.1).abs() < 0.001);
    assert_eq!((guest.y, guest.z), (host.y, host.z));
}

#[test]
fn the_floor_b2_jump_replays_the_lift_ride_writes_after_the_first_upgrade_state() {
    let steps = loadout_steps(StoryJumpPoint::JunkHeapFloorB2);
    let upgraded = steps
        .iter()
        .position(|step| *step == JumpStep::SetFlag { id: 1314 })
        .expect("upgrade flag");
    let cleared = steps
        .iter()
        .position(|step| *step == JumpStep::ClearFlag { id: 1314 })
        .expect("upgrade flag cleared");
    let lift = steps
        .iter()
        .position(|step| *step == JumpStep::SetFlag { id: 302 })
        .expect("lift flag");
    assert!(upgraded < cleared && cleared < lift);
}

#[test]
fn the_boss_fight_3_jump_keeps_the_whole_arena_loadout_and_ends_with_the_mp_step() {
    let arena = loadout_steps(StoryJumpPoint::AtShrineBoss);
    let fight = loadout_steps(StoryJumpPoint::BossFight3);
    let arena_without_mp = &arena[..arena.len() - 1];
    assert_eq!(&fight[..arena_without_mp.len()], arena_without_mp);
    assert_eq!(fight.last(), Some(&JumpStep::MpFullMode { on: false }));
    assert_eq!(
        fight
            .iter()
            .filter(|step| matches!(step, JumpStep::MpFullMode { .. }))
            .count(),
        1
    );
}

#[test]
fn the_boss_fight_3_jump_leaves_the_flags_that_start_the_fight_on_arrival() {
    let (flags, counters, snow) = final_story_state(loadout_steps(StoryJumpPoint::BossFight3));
    for id in [
        200, 201, 202, 217, 218, 219, 1361, 130, 195, 196, 198, 204, 220, 221, 222, 224, 225, 239,
        70, 79, 240, 241, 242, 255, 1442, 1451, 243, 1402,
    ] {
        assert_eq!(flags.get(&id), Some(&true), "{id}");
    }
    for id in [
        244, 245, 246, 247, 248, 249, 374, 387, 1304, 1305, 1401, 1403, 1407, 1408, 1409, 1433,
        1434,
    ] {
        assert_ne!(flags.get(&id), Some(&true), "{id}");
    }
    assert_eq!(counters.get(&100), Some(&255));
    assert_eq!(counters.get(&101), Some(&255));
    assert!(snow.contains(&1085));
}

#[test]
fn the_boss_fight_3_jump_gives_the_spells_and_tutorials_the_story_gave_before_it() {
    let steps = loadout_steps(StoryJumpPoint::BossFight3);
    for expected in [
        JumpStep::LearnMagic { id: 3 },
        JumpStep::LearnMagic { id: 6 },
        JumpStep::SetMagicSlot { slot: 0, magic: 6 },
        JumpStep::SetTutorial { id: 2005 },
        JumpStep::SetTutorial { id: 2020 },
        JumpStep::SetTutorial { id: 2079 },
        JumpStep::SetTutorial { id: 2014 },
    ] {
        assert!(steps.contains(&expected), "{expected:?}");
    }
    assert!(!steps.contains(&JumpStep::LearnMagic { id: 5 }));
}

#[test]
fn the_boss_fight_3_jump_lands_at_the_first_start_point_of_its_area() {
    let (name, point) = destination(StoryJumpPoint::BossFight3);
    assert_eq!(name, "A_CLIFF_VILLAGE_01");
    assert_eq!(point, 1);
    assert!(PhaseEntry::find(name).is_some());
}

#[test]
fn an_arrival_in_the_same_phase_is_due_once_the_map_was_left_and_has_settled() {
    let settled = LEVEL_CHECK_SETTLE_MS;
    assert!(!arrival_is_due("A", "A", false, true, settled));
    assert!(arrival_is_due("A", "A", true, true, settled));
    assert!(!arrival_is_due("A", "A", true, false, settled));
    assert!(!arrival_is_due("A", "A", true, true, settled - 1));
    assert!(arrival_is_due("A", "B", false, true, settled));
}

fn final_story_state(
    steps: Vec<JumpStep>,
) -> (
    std::collections::HashMap<u16, bool>,
    std::collections::HashMap<u8, u8>,
    std::collections::HashSet<u16>,
) {
    let mut flags = std::collections::HashMap::new();
    let mut counters = std::collections::HashMap::new();
    let mut snow = std::collections::HashSet::new();
    for step in steps {
        match step {
            JumpStep::SetFlag { id } => {
                flags.insert(id, true);
            }
            JumpStep::ClearFlag { id } => {
                flags.insert(id, false);
            }
            JumpStep::SetGameCounter { id, value } => {
                counters.insert(id, value);
            }
            JumpStep::SetSnowFlag { id } => {
                snow.insert(id);
            }
            _ => {}
        }
    }
    (flags, counters, snow)
}

#[test]
fn the_junk_heap_jump_keeps_the_whole_boss_fight_3_loadout_and_ends_with_the_mp_step() {
    let fight = loadout_steps(StoryJumpPoint::BossFight3);
    let junk_heap = loadout_steps(StoryJumpPoint::JunkHeapFloorB2);
    let fight_without_mp = &fight[..fight.len() - 1];
    assert_eq!(&junk_heap[..fight_without_mp.len()], fight_without_mp);
    assert_eq!(junk_heap.last(), Some(&JumpStep::MpFullMode { on: false }));
    assert_eq!(
        junk_heap
            .iter()
            .filter(|step| matches!(step, JumpStep::MpFullMode { .. }))
            .count(),
        1
    );
}

#[test]
fn the_junk_heap_jump_leaves_the_story_flags_a_played_run_has_on_floor_b2() {
    let (flags, counters, snow) = final_story_state(loadout_steps(StoryJumpPoint::JunkHeapFloorB2));
    for id in [
        70, 79, 129, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 253, 255, 256, 281, 282,
        283, 284, 285, 286, 287, 288, 290, 291, 292, 301, 302,
    ] {
        assert_eq!(flags.get(&id), Some(&true), "{id}");
    }
    for id in [
        1401, 1402, 1403, 1404, 1405, 1406, 1407, 1408, 1409, 1432, 1433, 1434, 1442, 1451, 137,
        252, 1317, 1314,
    ] {
        assert_ne!(flags.get(&id), Some(&true), "{id}");
    }
    assert_eq!(counters.get(&100), Some(&255));
    assert_eq!(counters.get(&101), Some(&255));
    assert!(snow.contains(&1085));
    assert!(snow.contains(&1051));
    assert!(snow.contains(&5));
}

#[test]
fn the_junk_heap_jump_leaves_out_the_side_quest_flags_and_counters() {
    let steps = loadout_steps(StoryJumpPoint::JunkHeapFloorB2);
    for step in &steps {
        match step {
            JumpStep::SetFlag { id } | JumpStep::ClearFlag { id } => {
                assert!(
                    !matches!(*id, 331 | 332 | 333 | 335) && !(1500..1700).contains(id),
                    "{id}"
                );
            }
            JumpStep::SetGameCounter { id, .. } => assert!(*id >= 100, "{id}"),
            _ => {}
        }
    }
}

#[test]
fn the_junk_heap_jump_gives_the_items_the_story_gave_and_the_upgraded_weapon() {
    let steps = loadout_steps(StoryJumpPoint::JunkHeapFloorB2);
    for expected in [
        JumpStep::SetItem { id: 1009, count: 1 },
        JumpStep::SetItem { id: 1109, count: 1 },
        JumpStep::EquipWeapon { id: 1109 },
        JumpStep::SetItem { id: 515, count: 1 },
        JumpStep::SetItem { id: 531, count: 1 },
        JumpStep::SetItem { id: 715, count: 1 },
        JumpStep::SetItem { id: 716, count: 1 },
        JumpStep::SetItem { id: 250, count: 1 },
        JumpStep::LearnMagic { id: 5 },
    ] {
        assert!(steps.contains(&expected), "{expected:?}");
    }
    for id in [2004, 2021, 2022, 2055] {
        assert!(steps.contains(&JumpStep::SetTutorial { id }), "{id}");
    }
    let weapon_given = steps
        .iter()
        .position(|step| *step == JumpStep::SetItem { id: 1009, count: 1 })
        .expect("weapon given");
    let weapon_upgraded = steps
        .iter()
        .position(|step| *step == JumpStep::SetItem { id: 1109, count: 1 })
        .expect("weapon upgraded");
    assert!(weapon_given < weapon_upgraded);
}

#[test]
fn the_junk_heap_jump_lands_on_the_first_start_point_of_the_second_robot_floor() {
    let (name, point) = destination(StoryJumpPoint::JunkHeapFloorB2);
    assert_eq!(name, "A_MOUNT_ROBOT_02");
    assert_eq!(point, 1);
    assert!(PhaseEntry::find(name).is_some());
}

#[test]
fn the_boss_fight_5_jump_keeps_the_floor_b2_loadout_and_ends_with_the_mp_step() {
    let floor = loadout_steps(StoryJumpPoint::JunkHeapFloorB2);
    let fight = loadout_steps(StoryJumpPoint::JunkHeapBossFight5);
    let floor_without_mp = &floor[..floor.len() - 1];
    assert_eq!(&fight[..floor_without_mp.len()], floor_without_mp);
    assert_eq!(fight.last(), Some(&JumpStep::MpFullMode { on: false }));
    assert_eq!(
        fight
            .iter()
            .filter(|step| matches!(step, JumpStep::MpFullMode { .. }))
            .count(),
        1
    );
}

#[test]
fn the_boss_fight_5_jump_leaves_the_flags_and_tutorials_the_bomb_and_truck_stretch_sets() {
    let steps = loadout_steps(StoryJumpPoint::JunkHeapBossFight5);
    let (flags, _, snow) = final_story_state(steps.clone());
    for id in [302, 303, 304, 305, 313, 314, 315] {
        assert_eq!(flags.get(&id), Some(&true), "{id}");
    }
    for id in [1401, 1402, 1403, 1404, 1405, 1406, 1407, 1314] {
        assert_ne!(flags.get(&id), Some(&true), "{id}");
    }
    assert!(snow.contains(&5));
    for id in [2050, 2052] {
        assert!(steps.contains(&JumpStep::SetTutorial { id }), "{id}");
    }
    assert!(!steps.contains(&JumpStep::SetTutorial { id: 2019 }));
}

#[test]
fn the_boss_fight_5_jump_arrives_at_the_first_start_point_of_the_last_robot_floor() {
    let (name, point) = destination(StoryJumpPoint::JunkHeapBossFight5);
    assert_eq!(name, "A_MOUNT_ROBOT_10");
    assert_eq!(point, 1);
    assert!(PhaseEntry::find(name).is_some());
}

#[test]
fn the_after_boss_fight_6_jump_keeps_the_boss_fight_5_loadout_and_ends_with_the_mp_step() {
    let earlier = loadout_steps(StoryJumpPoint::JunkHeapBossFight5);
    let after = loadout_steps(StoryJumpPoint::AfterBossFight6);
    let earlier_without_mp = &earlier[..earlier.len() - 1];
    assert_eq!(&after[..earlier_without_mp.len()], earlier_without_mp);
    assert_eq!(after.last(), Some(&JumpStep::MpFullMode { on: false }));
    assert_eq!(
        after
            .iter()
            .filter(|step| matches!(step, JumpStep::MpFullMode { .. }))
            .count(),
        1
    );
}

#[test]
fn the_after_boss_fight_6_jump_leaves_the_flags_a_played_run_has_after_the_ending_scene() {
    let (flags, counters, snow) = final_story_state(loadout_steps(StoryJumpPoint::AfterBossFight6));
    for id in [
        102, 306, 307, 308, 309, 132, 310, 311, 374, 375, 376, 377, 378, 379, 380, 381, 382, 383,
        384, 385, 386, 387, 125, 128, 248, 249, 1409,
    ] {
        assert_eq!(flags.get(&id), Some(&true), "{id}");
    }
    for id in [
        1307, 1308, 1309, 1310, 1401, 1402, 1403, 1404, 1405, 1406, 1407, 1408, 1410, 1411,
    ] {
        assert_ne!(flags.get(&id), Some(&true), "{id}");
    }
    assert_eq!(counters.get(&100), Some(&255));
    assert_eq!(counters.get(&101), Some(&255));
    assert!(snow.contains(&1056));
    assert!(!snow.contains(&1061));
    assert!(!counters.contains_key(&200));
}

#[test]
fn the_after_boss_fight_6_jump_gives_the_spells_and_tutorials_of_both_fight_endings() {
    let steps = loadout_steps(StoryJumpPoint::AfterBossFight6);
    for expected in [
        JumpStep::LearnMagic { id: 8 },
        JumpStep::LearnMagic { id: 4 },
        JumpStep::SetTutorial { id: 2007 },
        JumpStep::SetTutorial { id: 2003 },
    ] {
        assert!(steps.contains(&expected), "{expected:?}");
    }
}

#[test]
fn the_after_boss_fight_6_jump_lands_in_the_area_of_the_fight() {
    let (name, point) = destination(StoryJumpPoint::AfterBossFight6);
    assert_eq!(name, "A_CLIFF_VILLAGE_01");
    assert_eq!(point, 1);
    assert!(PhaseEntry::find(name).is_some());
}

#[test]
fn the_after_boss_fight_6_jump_places_the_host_where_the_ending_scene_leaves_it_and_the_guest_beside_it()
 {
    let host = arrival_spot(StoryJumpPoint::AfterBossFight6, true).expect("host spot");
    let guest = arrival_spot(StoryJumpPoint::AfterBossFight6, false).expect("guest spot");
    assert_eq!((host.x, host.y, host.z), (26.25, 30.61, 107.67));
    assert!((guest.x - host.x - 1.5).abs() < 0.001);
    assert_eq!((guest.y, guest.z), (host.y, host.z));
}

#[test]
fn the_boss_fight_6_jump_keeps_the_boss_fight_5_loadout_and_ends_with_the_mp_step() {
    let earlier = loadout_steps(StoryJumpPoint::JunkHeapBossFight5);
    let fight = loadout_steps(StoryJumpPoint::BossFight6);
    let earlier_without_mp = &earlier[..earlier.len() - 1];
    assert_eq!(&fight[..earlier_without_mp.len()], earlier_without_mp);
    assert_eq!(fight.last(), Some(&JumpStep::MpFullMode { on: false }));
    assert_eq!(
        fight
            .iter()
            .filter(|step| matches!(step, JumpStep::MpFullMode { .. }))
            .count(),
        1
    );
}

#[test]
fn the_after_boss_fight_6_jump_is_the_boss_fight_6_loadout_plus_the_fight() {
    let fight = loadout_steps(StoryJumpPoint::BossFight6);
    let after = loadout_steps(StoryJumpPoint::AfterBossFight6);
    let fight_without_mp = &fight[..fight.len() - 1];
    assert_eq!(&after[..fight_without_mp.len()], fight_without_mp);
    assert_eq!(after.len(), fight.len() + 50);
}

#[test]
fn the_boss_fight_6_jump_leaves_the_flags_that_make_the_area_start_run_the_approach_to_the_fight() {
    let (flags, counters, snow) = final_story_state(loadout_steps(StoryJumpPoint::BossFight6));
    for id in [102, 306, 307, 308, 309, 310, 311, 132, 374, 248, 249] {
        assert_eq!(flags.get(&id), Some(&true), "{id}");
    }
    for id in [
        375, 376, 377, 378, 379, 380, 381, 382, 383, 384, 385, 386, 387, 1307, 1308, 1309, 1310,
        1401, 1402, 1403, 1404, 1405, 1406, 1407, 1408, 1409, 1410, 1411, 125,
    ] {
        assert_ne!(flags.get(&id), Some(&true), "{id}");
    }
    assert_eq!(counters.get(&100), Some(&255));
    assert_eq!(counters.get(&101), Some(&255));
    assert!(snow.contains(&1056));
    assert!(!counters.contains_key(&200));
}

#[test]
fn the_boss_fight_6_jump_gives_the_spell_of_boss_fight_5_but_not_the_one_after_boss_fight_6() {
    let steps = loadout_steps(StoryJumpPoint::BossFight6);
    assert!(steps.contains(&JumpStep::LearnMagic { id: 8 }));
    assert!(steps.contains(&JumpStep::SetTutorial { id: 2007 }));
    assert!(!steps.contains(&JumpStep::LearnMagic { id: 4 }));
    assert!(!steps.contains(&JumpStep::SetTutorial { id: 2003 }));
}

#[test]
fn the_boss_fight_6_jump_enters_the_area_at_the_point_the_field_exit_uses() {
    let (name, point) = destination(StoryJumpPoint::BossFight6);
    assert_eq!(name, "A_CLIFF_VILLAGE_01");
    assert_eq!(point, 1);
    assert!(PhaseEntry::find(name).is_some());
}
