use replicant_game::Position;
use replicant_game::kaine::{
    GuestSceneStandIn, SCENE_STAND_INS, ScenePlaceSource, SceneStandInEvent, SceneStandInFacts,
    StandInSide, scene_stand_in_for_talk, scene_stand_in_place,
};
use replicant_game::puppet::PuppetPose;

const GATE_TALK: u32 = 1207;
const GATE_PACK: &str = "A_NORTH_FIELD_01";

fn guest_facts(now_ms: u64) -> SceneStandInFacts<'static> {
    SceneStandInFacts {
        side: StandInSide::Guest,
        in_session: true,
        pack: Some(GATE_PACK),
        closing_flag_on: false,
        free_to_move: true,
        now_ms,
    }
}

fn places(events: &[SceneStandInEvent]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, SceneStandInEvent::Place { .. }))
        .count()
}

fn release_reason(events: &[SceneStandInEvent]) -> Option<&'static str> {
    events.iter().find_map(|event| match event {
        SceneStandInEvent::Released { why, .. } => Some(*why),
        _ => None,
    })
}

#[test]
fn the_gate_talk_is_the_one_listed_scene() {
    assert_eq!(SCENE_STAND_INS.len(), 1);
    let row = scene_stand_in_for_talk(GATE_TALK).expect("the gate talk is listed");
    assert_eq!(row.pack, GATE_PACK);
    assert_eq!(row.ended_by, &["EID_1050_a0088g_End"]);
    assert_eq!(row.closed_by_game_flag, 389);
    assert_eq!(row.script_spot, [153.85, 9.57, 148.89]);
    assert!(scene_stand_in_for_talk(1208).is_none());
}

#[test]
fn the_host_talk_places_the_guest_once_and_its_end_releases_it() {
    let mut rule = GuestSceneStandIn::new();
    rule.partner_talk_line(GATE_TALK, 1_000);
    let opened = rule.step(&guest_facts(1_016));
    assert!(matches!(opened[0], SceneStandInEvent::Opened { .. }));
    assert_eq!(places(&opened), 1);
    rule.partner_talk_line(GATE_TALK, 2_000);
    assert_eq!(places(&rule.step(&guest_facts(2_016))), 0);
    rule.partner_talk_ended();
    let ended = rule.step(&guest_facts(14_000));
    assert_eq!(release_reason(&ended), Some("the host's talk ended"));
    assert!(matches!(
        ended[0],
        SceneStandInEvent::Released { placed: true, .. }
    ));
    assert!(rule.open_scene().is_none());
}

#[test]
fn another_talk_places_nobody() {
    let mut rule = GuestSceneStandIn::new();
    rule.partner_talk_line(1225, 0);
    assert!(rule.step(&guest_facts(16)).is_empty());
    assert!(rule.open_scene().is_none());
}

#[test]
fn a_busy_guest_waits_and_is_placed_when_free() {
    let mut rule = GuestSceneStandIn::new();
    rule.partner_talk_line(GATE_TALK, 0);
    let busy = SceneStandInFacts {
        free_to_move: false,
        ..guest_facts(16)
    };
    assert_eq!(places(&rule.step(&busy)), 0);
    assert_eq!(places(&rule.step(&guest_facts(500))), 1);
}

#[test]
fn a_guest_in_another_area_is_not_placed_and_still_released() {
    let mut rule = GuestSceneStandIn::new();
    rule.partner_talk_line(GATE_TALK, 0);
    let elsewhere = SceneStandInFacts {
        pack: Some("A_CENTER_VILLAGE_01"),
        ..guest_facts(16)
    };
    assert_eq!(places(&rule.step(&elsewhere)), 0);
    rule.partner_talk_ended();
    let ended = rule.step(&elsewhere);
    assert!(matches!(
        ended[0],
        SceneStandInEvent::Released { placed: false, .. }
    ));
}

#[test]
fn the_host_never_places_itself() {
    let mut rule = GuestSceneStandIn::new();
    rule.partner_talk_line(GATE_TALK, 0);
    let host = SceneStandInFacts {
        side: StandInSide::Host,
        ..guest_facts(16)
    };
    let events = rule.step(&host);
    assert_eq!(places(&events), 0);
    assert!(rule.open_scene().is_none());
}

#[test]
fn the_release_needs_nothing_from_the_guest() {
    let closers: [(
        &str,
        fn(&mut GuestSceneStandIn) -> SceneStandInFacts<'static>,
    ); 4] = [
        ("the scene's end ran here", |rule| {
            rule.function_ran("EID_1050_a0088g_End");
            guest_facts(5_000)
        }),
        ("the host's closing flag is on", |_| SceneStandInFacts {
            closing_flag_on: true,
            ..guest_facts(5_000)
        }),
        ("the session ended", |_| SceneStandInFacts {
            in_session: false,
            ..guest_facts(5_000)
        }),
        ("open too long without an end, let go", |_| {
            guest_facts(10_000_000)
        }),
    ];
    for (why, close) in closers {
        let mut rule = GuestSceneStandIn::new();
        rule.partner_talk_line(GATE_TALK, 0);
        rule.step(&guest_facts(16));
        let facts = SceneStandInFacts {
            free_to_move: false,
            ..close(&mut rule)
        };
        assert_eq!(release_reason(&rule.step(&facts)), Some(why), "{why}");
        assert!(rule.open_scene().is_none());
    }
}

#[test]
fn the_place_is_her_actor_when_the_script_has_put_her_there() {
    let row = scene_stand_in_for_talk(GATE_TALK).expect("listed");
    let her = PuppetPose {
        position: Position {
            x: 153.9,
            y: 9.6,
            z: 148.9,
        },
        yaw: -0.07,
    };
    let place = scene_stand_in_place(row, Some(her));
    assert_eq!(place.source, ScenePlaceSource::HerActor);
    assert_eq!(place.spot, [153.9, 9.6, 148.9]);
    assert!((place.heading_degrees - row.script_heading_degrees).abs() < 0.1);
}

#[test]
fn the_place_falls_back_to_the_script_when_she_is_elsewhere_or_unreadable() {
    let row = scene_stand_in_for_talk(GATE_TALK).expect("listed");
    let before_the_script_moved_her = PuppetPose {
        position: Position {
            x: -188.7,
            y: 35.4,
            z: 40.0,
        },
        yaw: -1.51,
    };
    let falling = PuppetPose {
        position: Position {
            x: f32::NAN,
            y: -14.1,
            z: f32::NAN,
        },
        yaw: f32::NAN,
    };
    for her in [Some(before_the_script_moved_her), Some(falling), None] {
        let place = scene_stand_in_place(row, her);
        assert_eq!(place.source, ScenePlaceSource::Script);
        assert_eq!(place.spot, row.script_spot);
        assert_eq!(place.heading_degrees, row.script_heading_degrees);
    }
}
