use replicant_game::stay_together::{
    JumpFacts, JumpPartner, JumpStep, SCRIPT_JUMP_SETTLE_MS, ScriptJumpWatch,
};

const START: [f32; 3] = [-54.0, 0.0, 40.0];
const LIFT_END: [f32; 3] = [-19.2, -0.1, -64.0];
const FAR_PARTNER: JumpPartner = JumpPartner {
    position: START,
    phase: 12,
};

fn facts(now_ms: u64, position: [f32; 3]) -> JumpFacts {
    JumpFacts {
        now_ms,
        position: Some(position),
        phase: Some(12),
        script_pause_in_force: true,
        mod_moved_the_host: false,
        partner: Some(FAR_PARTNER),
    }
}

fn watch_after_lift_jump() -> ScriptJumpWatch {
    let mut watch = ScriptJumpWatch::new();
    assert_eq!(watch.observe(&facts(0, START)), JumpStep::Nothing);
    assert_eq!(watch.observe(&facts(16, LIFT_END)), JumpStep::Nothing);
    watch
}

#[test]
fn a_lift_like_jump_with_a_far_partner_brings_the_partner_once() {
    let mut watch = watch_after_lift_jump();
    assert_eq!(
        watch.observe(&facts(16 + SCRIPT_JUMP_SETTLE_MS - 1, LIFT_END)),
        JumpStep::Nothing
    );
    let step = watch.observe(&facts(16 + SCRIPT_JUMP_SETTLE_MS, LIFT_END));
    let JumpStep::BringPartner {
        from,
        to,
        partner_away,
    } = step
    else {
        panic!("expected a bring, got {step:?}");
    };
    assert_eq!((from, to), (START, LIFT_END));
    assert!(partner_away > 100.0);
    assert_eq!(
        watch.observe(&facts(16 + SCRIPT_JUMP_SETTLE_MS + 16, LIFT_END)),
        JumpStep::Nothing
    );
}

#[test]
fn a_partner_already_near_after_the_settle_is_left_alone() {
    let mut watch = watch_after_lift_jump();
    let mut late = facts(16 + SCRIPT_JUMP_SETTLE_MS, LIFT_END);
    late.partner = Some(JumpPartner {
        position: [-19.0, 0.0, -62.0],
        phase: 12,
    });
    assert_eq!(watch.observe(&late), JumpStep::Nothing);
}

#[test]
fn a_partner_in_another_phase_is_left_alone() {
    let mut watch = watch_after_lift_jump();
    let mut late = facts(16 + SCRIPT_JUMP_SETTLE_MS, LIFT_END);
    late.partner = Some(JumpPartner {
        position: START,
        phase: 3,
    });
    assert_eq!(watch.observe(&late), JumpStep::Nothing);
}

#[test]
fn a_phase_change_is_not_a_jump() {
    let mut watch = ScriptJumpWatch::new();
    watch.observe(&facts(0, START));
    let mut moved = facts(16, LIFT_END);
    moved.phase = Some(13);
    assert_eq!(watch.observe(&moved), JumpStep::Nothing);
    let mut late = facts(16 + SCRIPT_JUMP_SETTLE_MS, LIFT_END);
    late.phase = Some(13);
    assert_eq!(watch.observe(&late), JumpStep::Nothing);
}

#[test]
fn a_jump_without_a_script_pause_is_not_counted() {
    let mut watch = ScriptJumpWatch::new();
    let mut before = facts(0, START);
    before.script_pause_in_force = false;
    let mut after = facts(16, LIFT_END);
    after.script_pause_in_force = false;
    watch.observe(&before);
    watch.observe(&after);
    let mut late = facts(16 + SCRIPT_JUMP_SETTLE_MS, LIFT_END);
    late.script_pause_in_force = false;
    assert_eq!(watch.observe(&late), JumpStep::Nothing);
}

#[test]
fn a_move_the_mod_made_is_not_counted() {
    let mut watch = ScriptJumpWatch::new();
    watch.observe(&facts(0, START));
    let mut by_mod = facts(16, LIFT_END);
    by_mod.mod_moved_the_host = true;
    assert_eq!(watch.observe(&by_mod), JumpStep::Nothing);
    assert_eq!(
        watch.observe(&facts(16 + SCRIPT_JUMP_SETTLE_MS, LIFT_END)),
        JumpStep::Nothing
    );
}

#[test]
fn a_walk_under_a_script_pause_is_not_a_jump() {
    let mut watch = ScriptJumpWatch::new();
    watch.observe(&facts(0, START));
    watch.observe(&facts(16, [-53.5, 0.0, 40.0]));
    assert_eq!(
        watch.observe(&facts(16 + SCRIPT_JUMP_SETTLE_MS, [-53.5, 0.0, 40.0])),
        JumpStep::Nothing
    );
}

#[test]
fn a_second_jump_restarts_the_settle_and_brings_once_after_it() {
    let mut watch = watch_after_lift_jump();
    let second_at = 800;
    let second_end = [10.0, 0.0, -64.0];
    assert_eq!(
        watch.observe(&facts(second_at, second_end)),
        JumpStep::Nothing
    );
    assert_eq!(
        watch.observe(&facts(16 + SCRIPT_JUMP_SETTLE_MS, second_end)),
        JumpStep::Nothing
    );
    let step = watch.observe(&facts(second_at + SCRIPT_JUMP_SETTLE_MS, second_end));
    let JumpStep::BringPartner { from, to, .. } = step else {
        panic!("expected a bring, got {step:?}");
    };
    assert_eq!((from, to), (START, second_end));
}
