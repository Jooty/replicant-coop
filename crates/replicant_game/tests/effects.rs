use replicant_game::effects::{
    EffectNameTally, LiveEffects, MAX_LASTING_SENT_EFFECTS, MAX_LIVE_PUPPET_EFFECTS,
    PUPPET_EFFECT_MAX_LIFETIME_MS, RateWindow, SOUND_FALLOFF_FAR_UNITS, SOUND_FALLOFF_NEAR_UNITS,
    SentEffects, build_effect_call, effect_instance_address, effect_is_finished, finish_effect,
    match_anchor_offset, read_anchor_offset, read_effect, read_sound, sound_gain,
    world_effects_shown_line,
};
use replicant_game::offsets::{
    EFFECT_ATTACH_BONE_OFFSET, EFFECT_ATTACH_SECOND_BONE_OFFSET, EFFECT_HANDLE_GENERATION_SHIFT,
    EFFECT_INSTANCE_CHANGED_FLAG_OFFSET, EFFECT_INSTANCE_FINISHED_FLAG_OFFSET,
    EFFECT_INVALID_HANDLE, EFFECT_PARAMS_PLAYER_ONLY_FLAG_OFFSET,
    EFFECT_PARAMS_SHAKE_STRENGTH_OFFSET, EFFECT_POOL_GENERATION_OFFSET, EFFECT_POOL_RVA,
    EFFECT_POOL_SLOT_STRIDE,
};
use replicant_proto::{EFFECT_ATTACH_BYTES, EFFECT_PARAM_BYTES, EffectPlay};

use crate::fake_memory::{FAKE_MODULE_BASE, FakeMemory};

const CALL: u64 = 0x2_0000_0000;
const INSTANCE: u64 = 0x2_2000_0000;
const EFFECT_NAME: &str = "ef_ground_slam";
const BONE_NAME: &str = "Hand_R";
const SECOND_BONE_NAME: &str = "Spine";

fn game_effect() -> EffectPlay {
    let mut params = vec![7; EFFECT_PARAM_BYTES];
    params[EFFECT_PARAMS_SHAKE_STRENGTH_OFFSET..EFFECT_PARAMS_SHAKE_STRENGTH_OFFSET + 4]
        .copy_from_slice(&0.5_f32.to_le_bytes());
    params[EFFECT_PARAMS_PLAYER_ONLY_FLAG_OFFSET] = 1;
    EffectPlay {
        effect: EFFECT_NAME.to_owned(),
        params,
        attach: vec![9; EFFECT_ATTACH_BYTES],
        bone: BONE_NAME.to_owned(),
        second_bone: SECOND_BONE_NAME.to_owned(),
        handle: 0,
    }
}

fn memory_with_effect_call(effect: &EffectPlay) -> (FakeMemory, u64) {
    let call = build_effect_call(effect);
    let mut memory = FakeMemory::new();
    memory.put(CALL, &call.bytes);
    (memory, CALL + call.attach_offset as u64)
}

fn handle(index: u32, generation: u32) -> u32 {
    (generation << EFFECT_HANDLE_GENERATION_SHIFT) | index
}

fn memory_with_pool_slot(index: u32, generation: u32) -> FakeMemory {
    let mut memory = FakeMemory::new();
    let slot =
        FAKE_MODULE_BASE + u64::from(EFFECT_POOL_RVA) + u64::from(index) * EFFECT_POOL_SLOT_STRIDE;
    memory.put(slot, &INSTANCE.to_le_bytes());
    memory.put(
        slot + EFFECT_POOL_GENERATION_OFFSET,
        &generation.to_le_bytes(),
    );
    memory.put(INSTANCE + EFFECT_INSTANCE_FINISHED_FLAG_OFFSET, &[0, 0]);
    memory.put(INSTANCE + EFFECT_INSTANCE_CHANGED_FLAG_OFFSET, &[0]);
    memory
}

#[test]
fn a_read_effect_has_its_name_both_bones_and_the_parameters() {
    let (memory, attach) = memory_with_effect_call(&game_effect());
    let captured = read_effect(&memory, CALL, attach).unwrap().unwrap();
    assert_eq!(captured.effect, EFFECT_NAME);
    assert_eq!(captured.bone, BONE_NAME);
    assert_eq!(captured.second_bone, SECOND_BONE_NAME);
    assert_eq!(captured.params[8], 7);
    assert_eq!(captured.attach[8], 9);
}

#[test]
fn a_read_effect_drops_what_would_touch_the_casters_own_state() {
    let (memory, attach) = memory_with_effect_call(&game_effect());
    let captured = read_effect(&memory, CALL, attach).unwrap().unwrap();
    let shake = &captured.params[EFFECT_PARAMS_SHAKE_STRENGTH_OFFSET..][..4];
    assert_eq!(shake, &[0; 4]);
    assert_eq!(captured.params[EFFECT_PARAMS_PLAYER_ONLY_FLAG_OFFSET], 0);
    assert_eq!(&captured.params[4..8], &[0; 4]);
    assert_eq!(&captured.attach[EFFECT_ATTACH_BONE_OFFSET..][..4], &[0; 4]);
    assert_eq!(
        &captured.attach[EFFECT_ATTACH_SECOND_BONE_OFFSET..][..4],
        &[0; 4]
    );
}

#[test]
fn an_effect_without_bone_names_reads_empty_bones() {
    let mut effect = game_effect();
    effect.bone.clear();
    effect.second_bone.clear();
    let (memory, attach) = memory_with_effect_call(&effect);
    let captured = read_effect(&memory, CALL, attach).unwrap().unwrap();
    assert_eq!(captured.bone, "");
    assert_eq!(captured.second_bone, "");
}

#[test]
fn a_block_without_a_name_offset_is_not_an_effect() {
    let mut memory = FakeMemory::new();
    memory.put(CALL, &[0; EFFECT_PARAM_BYTES + EFFECT_ATTACH_BYTES]);
    let attach = CALL + EFFECT_PARAM_BYTES as u64;
    assert_eq!(read_effect(&memory, CALL, attach).unwrap(), None);
}

#[test]
fn a_rebuilt_call_reads_back_as_the_same_effect() {
    let (memory, attach) = memory_with_effect_call(&game_effect());
    let original = read_effect(&memory, CALL, attach).unwrap().unwrap();
    let (again_memory, again_attach) = memory_with_effect_call(&original.to_wire(0));
    let again = read_effect(&again_memory, CALL, again_attach)
        .unwrap()
        .unwrap();
    assert_eq!(again, original);
}

#[test]
fn a_sound_name_is_read_up_to_its_end() {
    let mut memory = FakeMemory::new();
    memory.put(CALL, b"se_swing\0garbage");
    assert_eq!(
        read_sound(&memory, CALL).unwrap().as_deref(),
        Some("se_swing")
    );
    assert_eq!(read_sound(&memory, 0).unwrap(), None);
}

#[test]
fn an_unterminated_name_is_refused() {
    let mut memory = FakeMemory::new();
    memory.put(CALL, &[b'a'; 80]);
    assert_eq!(read_sound(&memory, CALL).unwrap(), None);
}

#[test]
fn a_handle_finds_its_effect_only_while_the_generation_matches() {
    let memory = memory_with_pool_slot(5, 9);
    assert_eq!(
        effect_instance_address(&memory, handle(5, 9)).unwrap(),
        Some(INSTANCE)
    );
    assert_eq!(
        effect_instance_address(&memory, handle(5, 8)).unwrap(),
        None
    );
    assert_eq!(
        effect_instance_address(&memory, EFFECT_INVALID_HANDLE).unwrap(),
        None
    );
}

#[test]
fn finishing_an_effect_sets_the_flags_the_remove_command_sets() {
    let mut memory = memory_with_pool_slot(5, 9);
    assert!(finish_effect(&mut memory, handle(5, 9)).unwrap());
    assert_eq!(
        memory.get(INSTANCE + EFFECT_INSTANCE_FINISHED_FLAG_OFFSET, 2),
        vec![1, 0]
    );
    assert_eq!(
        memory.get(INSTANCE + EFFECT_INSTANCE_CHANGED_FLAG_OFFSET, 1),
        vec![1]
    );
}

#[test]
fn finishing_a_stale_handle_changes_nothing() {
    let mut memory = memory_with_pool_slot(5, 9);
    assert!(!finish_effect(&mut memory, handle(5, 1)).unwrap());
    assert_eq!(
        memory.get(INSTANCE + EFFECT_INSTANCE_FINISHED_FLAG_OFFSET, 2),
        vec![0, 0]
    );
}

#[test]
fn puppet_effects_expire_after_their_lifetime() {
    let mut live = LiveEffects::new();
    live.track(1, 11, 1000);
    live.track(2, 12, 5000);
    assert!(
        live.take_expired(1000 + PUPPET_EFFECT_MAX_LIFETIME_MS - 1)
            .is_empty()
    );
    assert_eq!(
        live.take_expired(1000 + PUPPET_EFFECT_MAX_LIFETIME_MS),
        vec![(1, 11)]
    );
    assert_eq!(live.len(), 1);
}

#[test]
fn the_oldest_puppet_effect_is_handed_back_when_the_list_is_full() {
    let mut live = LiveEffects::new();
    for handle in 0..MAX_LIVE_PUPPET_EFFECTS as u32 {
        assert_eq!(live.track(handle, handle + 100, 0), None);
    }
    assert_eq!(live.track(1000, 2000, 0), Some(0));
    assert_eq!(live.len(), MAX_LIVE_PUPPET_EFFECTS);
    assert_eq!(live.take_all().len(), MAX_LIVE_PUPPET_EFFECTS);
    assert!(live.is_empty());
}

#[test]
fn a_rate_window_admits_its_limit_and_counts_the_rest() {
    let mut window = RateWindow::new(1000, 2);
    assert!(window.admit(0));
    assert!(window.admit(10));
    assert!(!window.admit(20));
    assert!(!window.admit(30));
    assert_eq!(window.take_refused(40), 2);
    assert_eq!(window.take_refused(50), 0);
    assert!(window.admit(1000));
}

#[test]
fn a_puppet_effect_is_found_by_the_senders_handle() {
    let mut live = LiveEffects::new();
    live.track(1, 11, 0);
    live.track(2, 12, 0);
    assert_eq!(live.take_by_sender(12), Some(2));
    assert_eq!(live.take_by_sender(12), None);
    assert_eq!(live.len(), 1);
}

#[test]
fn sent_effects_are_handed_back_once_they_end() {
    let mut sent = SentEffects::new();
    sent.track(1);
    sent.track(2);
    assert_eq!(sent.take_ended(|handle| handle == 2), vec![2]);
    assert_eq!(sent.len(), 1);
    assert!(sent.take_ended(|_| false).is_empty());
}

#[test]
fn a_lasting_effect_is_not_pushed_out_by_a_flood_of_short_ones() {
    let mut sent = SentEffects::new();
    sent.track_lasting(7, Some(0x1000));
    for handle in 100..100 + 2 * MAX_LIVE_PUPPET_EFFECTS as u32 {
        sent.track(handle);
    }
    assert_eq!(sent.take_ended(|handle| handle == 7), vec![7]);
}

#[test]
fn a_full_list_hands_back_the_oldest_handle_so_a_stop_can_be_sent() {
    let mut sent = SentEffects::new();
    for handle in 0..MAX_LIVE_PUPPET_EFFECTS as u32 {
        assert_eq!(sent.track(handle), None);
    }
    assert_eq!(sent.track(1000), Some(0));
    for handle in 0..MAX_LASTING_SENT_EFFECTS as u32 {
        assert_eq!(sent.track_lasting(handle, None), None);
    }
    assert_eq!(sent.track_lasting(5000, None), Some(0));
    assert_eq!(
        sent.len(),
        MAX_LIVE_PUPPET_EFFECTS + MAX_LASTING_SENT_EFFECTS
    );
}

#[test]
fn lasting_effects_are_handed_back_when_their_actor_is_gone() {
    let mut sent = SentEffects::new();
    sent.track_lasting(1, Some(0xA));
    sent.track_lasting(2, Some(0xB));
    sent.track_lasting(3, None);
    sent.track(4);
    assert_eq!(sent.take_anchor_gone(|actor| actor == 0xA), vec![1]);
    assert_eq!(sent.take_anchor_gone(|_| true), vec![2]);
    assert_eq!(sent.len(), 2);
}

#[test]
fn taking_everything_empties_both_lists() {
    let mut sent = SentEffects::new();
    sent.track(1);
    sent.track_lasting(2, None);
    let mut all = sent.take_all();
    all.sort_unstable();
    assert_eq!(all, vec![1, 2]);
    assert!(sent.is_empty());
}

#[test]
fn an_effect_is_finished_when_gone_or_flagged() {
    let mut memory = memory_with_pool_slot(5, 9);
    assert!(!effect_is_finished(&memory, handle(5, 9)).unwrap());
    assert!(effect_is_finished(&memory, handle(5, 1)).unwrap());
    finish_effect(&mut memory, handle(5, 9)).unwrap();
    assert!(effect_is_finished(&memory, handle(5, 9)).unwrap());
}

#[test]
fn a_sound_is_full_when_near_and_silent_when_far() {
    assert_eq!(sound_gain(0.0), 1.0);
    assert_eq!(sound_gain(SOUND_FALLOFF_NEAR_UNITS), 1.0);
    assert_eq!(sound_gain(SOUND_FALLOFF_FAR_UNITS), 0.0);
    assert_eq!(sound_gain(SOUND_FALLOFF_FAR_UNITS * 3.0), 0.0);
}

#[test]
fn a_sound_gets_quieter_with_every_step_away() {
    let middle = (SOUND_FALLOFF_NEAR_UNITS + SOUND_FALLOFF_FAR_UNITS) / 2.0;
    assert!(sound_gain(middle) < 1.0 && sound_gain(middle) > 0.0);
    assert!(sound_gain(middle + 5.0) < sound_gain(middle));
}

#[test]
fn a_puppet_takes_the_players_anchor_offset() {
    let mut memory = FakeMemory::new();
    let (player, puppet) = (CALL, INSTANCE);
    memory.put(player + 0x1C0, &[0; 12]);
    let mut raised = Vec::new();
    for value in [0.0_f32, 10.1, 0.0] {
        raised.extend_from_slice(&value.to_le_bytes());
    }
    memory.put(puppet + 0x1C0, &raised);
    let before = match_anchor_offset(&mut memory, player, puppet).unwrap();
    assert_eq!(before, [0.0, 10.1, 0.0]);
    assert_eq!(read_anchor_offset(&memory, puppet).unwrap(), [0.0; 3]);
}

#[test]
fn the_world_effects_line_names_each_effect_with_how_many_were_shown() {
    let mut tally = EffectNameTally::default();
    assert_eq!(world_effects_shown_line(&mut tally, 8), None);
    tally.note("efE_beam01", true);
    tally.note("efE_beam01", false);
    tally.note("efE_beam01", true);
    tally.note("efC_null", true);
    assert_eq!(
        world_effects_shown_line(&mut tally, 8).as_deref(),
        Some(
            "effects: last 5 s world effects from the partner by name: efE_beam01 x3 (2 shown, 1 not created), efC_null x1 (all shown)"
        )
    );
    assert!(tally.is_empty());
}
