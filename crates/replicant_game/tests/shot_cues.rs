use replicant_game::bullet_flights::{
    CueOnCopy, NO_COPY_TO_END, NO_COPY_TO_FOLLOW, ShotCueBoard, SightedBullet, one_shot_transform,
};
use replicant_game::enemy_bullets::{
    CueAdmission, EnemyBullet, REPEAT_WITHIN_THE_GAP, SHOT_CUE_REPEAT_GAP_MS, ShotCueGate,
    ShotCueTally, ShotInstances, effect_cue_of_command, end_cue_of_command, sound_cue_of_command,
};
use replicant_proto::{BULLET_EFFECT_SLOTS, BulletLook, EnemyId, ShotCue, ShotCueKind};

const SHOT_LOOK_ID: i32 = 8032;
const IMPACT_SOUND_ID: u32 = 58025;

fn placed(x: f32, y: f32, z: f32) -> [f32; 12] {
    [1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, y, 0.0, 0.0, 1.0, z]
}

fn cue(instance: u16, kind: ShotCueKind, id: u32) -> ShotCue {
    ShotCue {
        instance: Some(instance),
        kind,
        id,
        transform: placed(-18.3, 69.7, 0.6),
        offset: [0.0, -0.5, 0.0],
        style: [1.0; 8],
        sound_levels: [0.0; 2],
    }
}

fn sighted_shot(actor: u64, slot: u16) -> EnemyBullet {
    EnemyBullet {
        enemy: EnemyId {
            group: 0,
            ordinal: 1,
        },
        bullet: SightedBullet {
            actor,
            slot,
            hit_id: 1,
            transform: placed(20.0, 81.0, 3.0),
            look: BulletLook {
                effect_ids: [SHOT_LOOK_ID, 0, 0, 0],
                styles: [[1.0; 8]; BULLET_EFFECT_SLOTS],
            },
        },
        sprite: None,
        beam: None,
        scale_growth_per_second: 0.0,
    }
}

#[test]
fn a_look_slot_is_not_sent_twice() {
    assert_eq!(effect_cue_of_command(0x39), None);
    assert_eq!(effect_cue_of_command(0x3B), None);
    for one_shot in [0x36, 0x37, 0x38] {
        assert_eq!(
            effect_cue_of_command(one_shot),
            Some(ShotCueKind::OneShotEffect)
        );
    }
    assert_eq!(
        effect_cue_of_command(0x3D),
        Some(ShotCueKind::FollowingEffect)
    );
}

#[test]
fn a_shots_sounds_and_ends_are_read_from_their_command() {
    assert_eq!(sound_cue_of_command(0x41), ShotCueKind::OneShotSound);
    assert_eq!(sound_cue_of_command(0x42), ShotCueKind::OneShotSound);
    assert_eq!(sound_cue_of_command(0x43), ShotCueKind::LastingSound);
    assert_eq!(sound_cue_of_command(0x45), ShotCueKind::LastingSound);
    assert_eq!(end_cue_of_command(0x44), Some(ShotCueKind::EndLastingSound));
    assert_eq!(end_cue_of_command(0x46), Some(ShotCueKind::EndLastingSound));
    assert_eq!(
        end_cue_of_command(0x3E),
        Some(ShotCueKind::EndFollowingEffects)
    );
    assert_eq!(end_cue_of_command(0x41), None);
}

#[test]
fn a_shots_trail_is_made_on_the_copy() {
    let mut board = ShotCueBoard::default();
    let trail = cue(300, ShotCueKind::FollowingEffect, 8034);
    assert_eq!(board.note(trail, true), CueOnCopy::Follow { instance: 300 });
    assert_eq!(
        board.note(cue(300, ShotCueKind::EndFollowingEffects, 0), true),
        CueOnCopy::End { instance: 300 }
    );
    assert_eq!(
        board.note(trail, false),
        CueOnCopy::Drop {
            why: NO_COPY_TO_FOLLOW
        }
    );
    assert_eq!(
        board.note(cue(300, ShotCueKind::EndLastingSound, 0), false),
        CueOnCopy::Drop {
            why: NO_COPY_TO_END
        }
    );
}

#[test]
fn a_shots_impact_is_made_where_the_copy_ended() {
    let mut board = ShotCueBoard::default();
    let impact = cue(300, ShotCueKind::OneShotEffect, 8033);
    assert_eq!(board.note(impact, true), CueOnCopy::Wait);
    let copy_end = placed(-17.0, 70.2, 1.0);
    board.note_stop(300, copy_end);
    let due = board.take_due(|_| Some(placed(99.0, 99.0, 99.0)));
    assert_eq!(due.len(), 1);
    let (made, transform) = due[0];
    assert_eq!(made, impact);
    assert_eq!(
        [transform[3], transform[7], transform[11]],
        [-17.0, 69.7, 1.0]
    );
    assert!(board.take_due(|_| None).is_empty());
}

#[test]
fn an_impact_after_its_copy_ended_in_the_same_tick_still_lands_where_it_ended() {
    let mut board = ShotCueBoard::default();
    board.note_stop(300, placed(5.0, 70.0, 5.0));
    let sound = cue(300, ShotCueKind::OneShotSound, IMPACT_SOUND_ID);
    assert_eq!(board.note(sound, false), CueOnCopy::Wait);
    let due = board.take_due(|_| None);
    assert_eq!([due[0].1[3], due[0].1[7], due[0].1[11]], [5.0, 69.5, 5.0]);
}

#[test]
fn an_impact_with_no_copy_is_made_where_the_owner_made_it() {
    let mut board = ShotCueBoard::default();
    let impact = cue(301, ShotCueKind::OneShotEffect, 8033);
    assert_eq!(
        board.note(impact, false),
        CueOnCopy::Place {
            transform: impact.transform
        }
    );
    let unbound = ShotCue {
        instance: None,
        ..impact
    };
    assert_eq!(
        board.note(unbound, true),
        CueOnCopy::Place {
            transform: impact.transform
        }
    );
    assert_eq!(one_shot_transform(&impact, None), impact.transform);
}

#[test]
fn a_one_shot_on_a_live_copy_lands_on_the_copy_plus_the_owners_offset() {
    let mut board = ShotCueBoard::default();
    let puff = cue(302, ShotCueKind::OneShotEffect, 8035);
    assert_eq!(board.note(puff, true), CueOnCopy::Wait);
    let due = board.take_due(|instance| (instance == 302).then(|| placed(1.0, 2.0, 3.0)));
    assert_eq!([due[0].1[3], due[0].1[7], due[0].1[11]], [1.0, 1.5, 3.0]);
}

#[test]
fn a_burst_of_shots_keeps_every_impact() {
    let mut gate = ShotCueGate::default();
    let mut board = ShotCueBoard::default();
    let burst: u16 = 64;
    for shot in 0..burst {
        let admitted = gate.admit(
            0x1000 + u64::from(shot),
            ShotCueKind::OneShotEffect,
            8033,
            1000,
        );
        assert_eq!(admitted, CueAdmission::Send);
        let sound = gate.admit(
            0x1000 + u64::from(shot),
            ShotCueKind::OneShotSound,
            IMPACT_SOUND_ID,
            1000,
        );
        assert_eq!(sound, CueAdmission::Send);
        assert_eq!(
            board.note(cue(shot, ShotCueKind::OneShotEffect, 8033), true),
            CueOnCopy::Wait
        );
        board.note_stop(shot, placed(f32::from(shot), 69.7, 0.0));
    }
    assert_eq!(board.take_due(|_| None).len(), usize::from(burst));
}

#[test]
fn a_shots_repeated_puff_goes_once_per_gap_and_an_end_is_never_held() {
    let mut gate = ShotCueGate::default();
    let kind = ShotCueKind::OneShotEffect;
    assert_eq!(gate.admit(7, kind, 8035, 1000), CueAdmission::Send);
    assert_eq!(gate.admit(7, kind, 8035, 1016), CueAdmission::RepeatTooSoon);
    assert_eq!(gate.admit(8, kind, 8035, 1016), CueAdmission::Send);
    assert_eq!(gate.admit(7, kind, 8036, 1016), CueAdmission::Send);
    assert_eq!(
        gate.admit(7, kind, 8035, 1000 + SHOT_CUE_REPEAT_GAP_MS),
        CueAdmission::Send
    );
    for _ in 0..3 {
        assert_eq!(
            gate.admit(7, ShotCueKind::EndLastingSound, 0, 1100),
            CueAdmission::Send
        );
    }
}

#[test]
fn an_impact_made_after_its_bullet_left_the_scan_still_names_its_shot() {
    let mut instances = ShotInstances::default();
    instances.note_scan(&[sighted_shot(0xA0, 300), sighted_shot(0xB0, 301)]);
    instances.note_scan(&[sighted_shot(0xB0, 301)]);
    assert_eq!(instances.instance_of(0xA0), Some(300));
    assert_eq!(instances.instance_of(0xB0), Some(301));
    instances.note_scan(&[]);
    assert_eq!(instances.instance_of(0xA0), None);
    assert_eq!(instances.instance_of(0xB0), Some(301));
    assert_eq!(instances.instance_of(0xC0), None);
}

#[test]
fn the_shot_cue_line_says_what_was_made_and_what_was_dropped_and_why() {
    let mut tally = ShotCueTally::default();
    assert_eq!(tally.take_line("made", 0), None);
    assert_eq!(
        tally.take_line("made", 3).as_deref(),
        Some(
            "shot cues: last 1 s made 0 for 0 shots (0 trails, 0 impacts, 0 sounds, 0 ends, 0 resizes), 3 shots in the air, dropped 0"
        )
    );
    tally.note_cue(Some(300), ShotCueKind::FollowingEffect);
    tally.note_cue(Some(300), ShotCueKind::OneShotEffect);
    tally.note_cue(Some(301), ShotCueKind::OneShotSound);
    tally.note_cue(None, ShotCueKind::LastingSound);
    tally.note_cue(Some(301), ShotCueKind::EndLastingSound);
    tally.note_dropped(REPEAT_WITHIN_THE_GAP);
    tally.note_dropped(REPEAT_WITHIN_THE_GAP);
    tally.note_dropped(NO_COPY_TO_FOLLOW);
    assert_eq!(
        tally.take_line("sent", 2).as_deref(),
        Some(
            "shot cues: last 1 s sent 5 for 2 shots (1 trails, 1 impacts, 2 sounds, 1 ends, 0 resizes), 2 shots in the air, dropped 3 (2 a repeat within 50 ms, 1 with no copy to follow)"
        )
    );
    assert_eq!(tally.take_line("sent", 0), None);
}

#[test]
fn the_shot_cue_line_counts_resizes_with_the_largest_scale() {
    let mut tally = ShotCueTally::default();
    tally.note_resize(140, 2.6);
    tally.note_resize(140, 9.81);
    tally.note_resize(141, 4.6);
    assert_eq!(
        tally.take_line("made", 0).as_deref(),
        Some(
            "shot cues: last 1 s made 0 for 2 shots (0 trails, 0 impacts, 0 sounds, 0 ends, 3 resizes up to scale 9.8), 0 shots in the air, dropped 0"
        )
    );
}
