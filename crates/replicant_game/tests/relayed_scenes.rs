use replicant_game::callback_gate::{PlayedStart, RelayedScenesPlayed, scene_base, start_base};

const CLIFF_VILLAGE: u16 = 2;
const OTHER_PHASE: u16 = 3;
const FIGHT_6_SCENE: &str = "EID_1050_a0020_PLAY_EVENT";
const FIGHT_6_START: &str = "EID_1050_a0020_Start";
const FIGHT_6_END: &str = "EID_1050_a0020_End";
const FIGHT_6_SECOND_SCENE_START: &str = "EID_1050_a0030_Start";

#[test]
fn a_scene_function_names_its_event() {
    assert_eq!(scene_base(FIGHT_6_SCENE), Some("EID_1050_a0020"));
    assert_eq!(scene_base("EID_1050_a0030_EVENT"), Some("EID_1050_a0030"));
    assert_eq!(scene_base("EID_1050_a0060_Event"), Some("EID_1050_a0060"));
    assert_eq!(scene_base("RETRY_FLAG_ON_002"), None);
    assert_eq!(scene_base(FIGHT_6_START), None);
    assert_eq!(start_base(FIGHT_6_START), Some("EID_1050_a0020"));
    assert_eq!(start_base(FIGHT_6_END), None);
}

#[test]
fn run_1113g_the_hosts_late_start_after_the_relayed_scene_is_dropped() {
    // Host: the guest's scene ran here at 13:01:03.451, its own trap fired the start at 13:01:07.214.
    let mut played = RelayedScenesPlayed::new();
    assert!(!played.note_played(CLIFF_VILLAGE, "RETRY_FLAG_ON_002"));
    assert!(played.note_played(CLIFF_VILLAGE, FIGHT_6_SCENE));
    assert_eq!(
        played.judge_start(CLIFF_VILLAGE, FIGHT_6_START),
        PlayedStart::Drop {
            first_for_start: true
        }
    );
    assert_eq!(
        played.judge_start(CLIFF_VILLAGE, FIGHT_6_START),
        PlayedStart::Drop {
            first_for_start: false
        }
    );
}

#[test]
fn the_next_scene_of_the_fight_still_starts() {
    let mut played = RelayedScenesPlayed::new();
    played.note_played(CLIFF_VILLAGE, FIGHT_6_SCENE);
    assert_eq!(
        played.judge_start(CLIFF_VILLAGE, FIGHT_6_SECOND_SCENE_START),
        PlayedStart::Run
    );
    assert_eq!(
        played.judge_start(CLIFF_VILLAGE, FIGHT_6_END),
        PlayedStart::Run
    );
}

#[test]
fn a_start_with_no_relayed_scene_runs() {
    let mut played = RelayedScenesPlayed::new();
    assert_eq!(
        played.judge_start(CLIFF_VILLAGE, FIGHT_6_START),
        PlayedStart::Run
    );
}

#[test]
fn another_phase_or_a_reset_forgets_the_scene() {
    let mut played = RelayedScenesPlayed::new();
    played.note_played(CLIFF_VILLAGE, FIGHT_6_SCENE);
    assert!(!played.played(OTHER_PHASE, FIGHT_6_START));
    played.note_played(OTHER_PHASE, "EID_0100_a0010_PLAY_EVENT");
    assert!(!played.played(CLIFF_VILLAGE, FIGHT_6_START));
    played.note_played(CLIFF_VILLAGE, FIGHT_6_SCENE);
    played.reset();
    assert_eq!(
        played.judge_start(CLIFF_VILLAGE, FIGHT_6_START),
        PlayedStart::Run
    );
}
