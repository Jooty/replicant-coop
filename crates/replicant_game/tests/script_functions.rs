use replicant_game::callback_gate::{
    EndArrival, EndFollow, EndQueue, ParkedCall, ParkedKind, QueueEvent, Released,
    control_slots_free, end_follow,
};
use replicant_game::coop_pause::LEFTOVER_PAUSE_MENU_MASK;
use replicant_game::script_functions::{
    DEBUG_PRINT_TEXT_ARG_INDEX, DrainStep, FunctionArrival, FunctionArrivalFacts, FunctionDrop,
    FunctionsRan, MODAL_UI_FUNCTIONS, SCRIPT_FUNCTIONS, debug_print_args_len,
    debug_print_tag_range, debug_print_text_range, function_arrival, is_callee_of,
    plan_forced_drain, script_function, shows_modal_ui, traced_function_name, traced_function_text,
    traced_function_text_from_parts,
};
use replicant_game::shared_events::SHARED_EVENT_BASES;
use replicant_proto::StoryCall;

const LABEL: &str = "A_STONE_FIELD_02";
const PHASE: u16 = 7;
const TALK02: &str = "EID_0200_a0030g_Talk02";
const TALK02_END: &str = "EID_0200_a0030g_Talk02_End";
const TALK03: &str = "EID_0200_a0030g_Talk03";
const TALK03_END: &str = "EID_0200_a0030g_Talk03_End";
const TALK04: &str = "EID_0200_a0030g_Talk04";
const TALK04_END: &str = "EID_0200_a0030g_Talk04_End";
const SCENE_END: &str = "EID_0200_a0030g_End";
const CUTSCENE_START: &str = "EID_0200_a0032_Start";
const OPEN_PAUSE_MENU_SLOTS: [u32; 4] = [LEFTOVER_PAUSE_MENU_MASK, 0, 0, 0];

fn facts(queue_is_empty: bool) -> FunctionArrivalFacts<'static> {
    FunctionArrivalFacts {
        enabled: true,
        local_in_play: true,
        local_phase: Some(PHASE),
        call_phase: PHASE,
        phase_label: LABEL,
        queue_is_empty,
    }
}

struct Guest {
    queue: EndQueue,
    ran: FunctionsRan,
    pause_menu_open: bool,
    in_own_talk: bool,
    order: Vec<String>,
    menu_closed_when: Vec<String>,
    waited_on_menu: bool,
    superseded: Vec<String>,
    dropped: Vec<(String, FunctionDrop)>,
}

impl Guest {
    fn new(pause_menu_open: bool, in_own_talk: bool) -> Self {
        Self {
            queue: EndQueue::new(),
            ran: FunctionsRan::new(),
            pause_menu_open,
            in_own_talk,
            order: Vec::new(),
            menu_closed_when: Vec::new(),
            waited_on_menu: false,
            superseded: Vec::new(),
            dropped: Vec::new(),
        }
    }

    fn call(name: &str) -> ParkedCall {
        ParkedCall {
            kind: ParkedKind::Function,
            call: StoryCall {
                phase: PHASE,
                pack: 0,
                function: name.to_owned(),
                args: Vec::new(),
            },
        }
    }

    fn close_menu_if_modal(&mut self, name: &str) {
        if self.pause_menu_open && shows_modal_ui(LABEL, name) {
            self.pause_menu_open = false;
            self.menu_closed_when.push(name.to_owned());
        }
    }

    fn run_function(&mut self, name: &str) {
        let queued = self.queue.queued_function_names(PHASE);
        if let Some(reason) =
            replicant_game::script_functions::judge_function(LABEL, PHASE, name, &self.ran, &queued)
        {
            self.dropped.push((name.to_owned(), reason));
            return;
        }
        self.close_menu_if_modal(name);
        self.ran.note(PHASE, name);
        self.order.push(name.to_owned());
    }

    fn run_end(&mut self, name: &str) {
        self.close_menu_if_modal(name);
        self.ran.note(PHASE, name);
        self.order.push(name.to_owned());
    }

    fn run_released(&mut self, released: Released) {
        match released {
            Released::End { end, .. } => self.run_end(&end.name),
            Released::Call { parked, .. } => self.run_function(&parked.call.function),
        }
    }

    fn partner_end(&mut self, name: &str, now: u64) {
        self.queue.push(name, PHASE, now);
    }

    fn partner_function(&mut self, name: &str, now: u64) {
        let queued = self.queue.queued_function_names(PHASE);
        let verdict = function_arrival(&facts(self.queue.is_empty()), name, &self.ran, &queued);
        match verdict {
            FunctionArrival::Drop(reason) => self.dropped.push((name.to_owned(), reason)),
            FunctionArrival::RunNow => self.run_function(name),
            FunctionArrival::Park => {
                self.queue.park_call_always(Self::call(name), now);
            }
            FunctionArrival::ForcedDrain => {
                self.queue.park_call_always(Self::call(name), now);
                let taken = self.queue.take_through_function(name);
                self.close_menu_if_modal(name);
                for step in plan_forced_drain(taken, name, LABEL) {
                    match step {
                        DrainStep::Run(released) => self.run_released(released),
                        DrainStep::Superseded { parked, .. } => {
                            self.superseded.push(parked.call.function);
                        }
                    }
                }
            }
        }
    }

    fn tick(&mut self, now: u64) {
        let slots = if self.pause_menu_open {
            OPEN_PAUSE_MENU_SLOTS
        } else {
            [0; 4]
        };
        let slots_free = control_slots_free(slots, self.pause_menu_open);
        let in_own_talk = self.in_own_talk;
        let mut waited_on_menu = false;
        let events = self.queue.advance(now, |_end| {
            let verdict = end_follow(&EndArrival {
                message_phase: PHASE,
                shared_phase: Some(PHASE),
                local_phase: Some(PHASE),
                local_in_own_talk: in_own_talk,
                control_mask_settled: slots_free,
                already_ran_here: false,
                own_copy_of_same_event_running: false,
            });
            if verdict != EndFollow::Mirror && !in_own_talk {
                waited_on_menu = true;
            }
            verdict
        });
        self.waited_on_menu |= waited_on_menu;
        for event in events {
            match event {
                QueueEvent::Run { end, .. } => self.run_end(&end.name),
                QueueEvent::RunCall { parked, .. } => self.run_function(&parked.call.function),
                QueueEvent::Dropped { .. } | QueueEvent::StillWaiting { .. } => {}
            }
        }
    }
}

fn run31_host_stream(guest: &mut Guest) {
    guest.partner_function(TALK02, 6_021);
    guest.tick(6_100);
    guest.partner_end(TALK02_END, 7_598);
    guest.tick(7_700);
    guest.partner_function(TALK03, 9_300);
    guest.tick(9_400);
    guest.partner_end(TALK03_END, 12_061);
    guest.tick(12_100);
    guest.partner_function(TALK04, 13_800);
    guest.tick(13_900);
    guest.partner_end(TALK04_END, 14_776);
    guest.tick(14_800);
    guest.partner_function(SCENE_END, 15_466);
    guest.tick(15_500);
    guest.partner_function(CUTSCENE_START, 17_900);
    guest.tick(18_000);
}

#[test]
fn run31_with_the_guests_pause_menu_open_nothing_waits_and_the_menu_closes_only_for_the_scene() {
    let mut guest = Guest::new(true, false);
    run31_host_stream(&mut guest);
    assert_eq!(
        guest.order,
        [
            TALK02, TALK02_END, TALK03, TALK03_END, TALK04, TALK04_END, SCENE_END
        ]
    );
    assert!(!guest.waited_on_menu);
    assert_eq!(guest.menu_closed_when, [SCENE_END]);
    assert!(guest.queue.is_empty());
}

#[test]
fn the_cutscene_start_called_inside_the_scene_end_is_dropped_not_run_twice() {
    let mut guest = Guest::new(true, false);
    run31_host_stream(&mut guest);
    assert_eq!(
        guest.dropped,
        [(
            CUTSCENE_START.to_owned(),
            FunctionDrop::CalleeOf(SCENE_END.to_owned())
        )]
    );
    assert!(!guest.order.contains(&CUTSCENE_START.to_owned()));
}

#[test]
fn a_callee_that_arrives_while_its_caller_is_queued_is_dropped() {
    let mut guest = Guest::new(false, true);
    guest.partner_end(TALK02_END, 0);
    guest.partner_function(TALK03, 10);
    guest.partner_function(SCENE_END, 20);
    guest.partner_function(CUTSCENE_START, 30);
    assert!(guest.order.contains(&SCENE_END.to_owned()));
    assert!(!guest.order.contains(&CUTSCENE_START.to_owned()));
    assert!(matches!(
        guest.dropped.as_slice(),
        [(name, FunctionDrop::CalleeOf(_))] if name == CUTSCENE_START
    ));
}

#[test]
fn while_an_end_waits_for_the_players_own_talk_the_scene_drains_ends_and_supersedes_talk_functions()
{
    let mut guest = Guest::new(true, true);
    guest.partner_end(TALK02_END, 0);
    guest.tick(10);
    guest.partner_function(TALK03, 20);
    guest.partner_end(TALK03_END, 30);
    guest.partner_function(TALK04, 40);
    guest.partner_end(TALK04_END, 50);
    guest.tick(60);
    assert!(guest.order.is_empty());
    assert!(guest.pause_menu_open);
    guest.partner_function(SCENE_END, 70);
    assert_eq!(guest.order, [TALK02_END, TALK03_END, TALK04_END, SCENE_END]);
    assert_eq!(guest.superseded, [TALK03, TALK04]);
    assert_eq!(guest.menu_closed_when, [SCENE_END]);
    assert!(guest.queue.is_empty());
}

#[test]
fn the_control_mask_a_pause_menu_holds_does_not_count_as_busy() {
    assert!(control_slots_free(OPEN_PAUSE_MENU_SLOTS, true));
    assert!(!control_slots_free(OPEN_PAUSE_MENU_SLOTS, false));
    assert!(!control_slots_free([0, 0x80, 0, 0], true));
    assert!(!control_slots_free([0, 0, 0x2, 0], true));
    assert!(control_slots_free([0; 4], false));
}

#[test]
fn an_end_no_longer_waits_for_an_open_menu() {
    let arrival = EndArrival {
        message_phase: PHASE,
        shared_phase: Some(PHASE),
        local_phase: Some(PHASE),
        local_in_own_talk: false,
        control_mask_settled: control_slots_free(OPEN_PAUSE_MENU_SLOTS, true),
        already_ran_here: false,
        own_copy_of_same_event_running: false,
    };
    assert_eq!(end_follow(&arrival), EndFollow::Mirror);
}

#[test]
fn radio_and_flag_functions_do_not_show_modal_ui_but_the_scene_does() {
    for name in [TALK02, TALK03, TALK04, TALK02_END, TALK04_END] {
        assert!(!shows_modal_ui(LABEL, name), "{name}");
    }
    assert!(shows_modal_ui(LABEL, SCENE_END));
    assert!(shows_modal_ui(LABEL, CUTSCENE_START));
    assert!(script_function(LABEL, SCENE_END).is_some_and(|function| function.starts_scene));
}

#[test]
fn the_scene_end_reaches_the_cutscene_start_and_the_talks_are_plain_functions() {
    assert!(is_callee_of(LABEL, CUTSCENE_START, SCENE_END));
    assert!(!is_callee_of(LABEL, SCENE_END, CUTSCENE_START));
    assert!(!script_function(LABEL, TALK02).is_some_and(|function| function.starts_scene));
}

#[test]
fn the_hit_counter_is_not_a_shared_function_so_no_hit_is_relayed() {
    assert!(script_function(LABEL, "AttackCount_Talk").is_none());
    let verdict = function_arrival(&facts(true), "AttackCount_Talk", &FunctionsRan::new(), &[]);
    assert_eq!(
        verdict,
        FunctionArrival::Drop(FunctionDrop::NotASharedFunction)
    );
}

#[test]
fn a_function_that_ran_here_is_dropped_and_so_is_a_callee_of_one_that_ran() {
    let mut ran = FunctionsRan::new();
    ran.note(PHASE, TALK02);
    assert_eq!(
        function_arrival(&facts(true), TALK02, &ran, &[]),
        FunctionArrival::Drop(FunctionDrop::AlreadyRan)
    );
    ran.note(PHASE, SCENE_END);
    assert_eq!(
        function_arrival(&facts(true), CUTSCENE_START, &ran, &[]),
        FunctionArrival::Drop(FunctionDrop::CalleeOf(SCENE_END.to_owned()))
    );
    let queued = [SCENE_END.to_owned()];
    assert_eq!(
        function_arrival(&facts(false), CUTSCENE_START, &FunctionsRan::new(), &queued),
        FunctionArrival::Drop(FunctionDrop::CalleeOf(SCENE_END.to_owned()))
    );
}

#[test]
fn the_ran_list_belongs_to_one_phase_and_forgets_on_leaving_it() {
    let mut ran = FunctionsRan::new();
    assert!(ran.note(PHASE, TALK02));
    assert!(!ran.note(PHASE, TALK02));
    assert!(ran.contains(PHASE, TALK02));
    ran.enter_phase(Some(PHASE + 1));
    assert!(!ran.contains(PHASE, TALK02));
    assert!(ran.note(PHASE + 1, TALK02));
}

#[test]
fn arrivals_are_refused_when_this_game_cannot_take_them() {
    let ran = FunctionsRan::new();
    let mut disabled = facts(true);
    disabled.enabled = false;
    let mut away = facts(true);
    away.local_in_play = false;
    let mut elsewhere = facts(true);
    elsewhere.local_phase = Some(PHASE + 1);
    for (case, reason) in [
        (disabled, FunctionDrop::Disabled),
        (away, FunctionDrop::NotInPlay),
        (elsewhere, FunctionDrop::WrongPhase),
    ] {
        assert_eq!(
            function_arrival(&case, TALK02, &ran, &[]),
            FunctionArrival::Drop(reason)
        );
    }
}

#[test]
fn a_function_waits_behind_earlier_calls_but_runs_at_once_with_an_empty_queue() {
    let ran = FunctionsRan::new();
    assert_eq!(
        function_arrival(&facts(true), TALK02, &ran, &[]),
        FunctionArrival::RunNow
    );
    assert_eq!(
        function_arrival(&facts(false), TALK02, &ran, &[]),
        FunctionArrival::Park
    );
    assert_eq!(
        function_arrival(&facts(false), SCENE_END, &ran, &[]),
        FunctionArrival::ForcedDrain
    );
}

#[test]
fn both_print_forms_name_the_function_and_other_text_does_not() {
    assert_eq!(
        traced_function_name("A_STONE_FIELD_02->EID_0200_a0030g_Talk03"),
        Some(TALK03)
    );
    assert_eq!(
        traced_function_name("EID_0200_a0030g_End()"),
        Some(SCENE_END)
    );
    assert_eq!(
        traced_function_name("B_ENDING_01->EID_7000_a0020_End()"),
        Some("EID_7000_a0020_End")
    );
    for text in [
        "EID_0200_a0030g_End",
        "",
        "->",
        "()",
        "two words()",
        "hits 12",
    ] {
        assert_eq!(traced_function_name(text), None, "{text}");
    }
}

#[test]
fn the_tag_is_read_before_the_text_and_a_non_text_tag_skips_the_call() {
    let tag_range = debug_print_tag_range();
    let text_range = debug_print_text_range();
    assert_eq!(tag_range.len(), 4);
    assert_eq!(tag_range.end + 12, text_range.start);
    let text = b"A_STONE_FIELD_02->EID_0200_a0030g_Talk02\0";
    let mut padded = text.to_vec();
    padded.resize(text_range.len(), 0);
    assert_eq!(
        traced_function_text_from_parts(&2_i32.to_le_bytes(), &padded).as_deref(),
        Some("A_STONE_FIELD_02->EID_0200_a0030g_Talk02")
    );
    for tag in [0_i32, 1, 3, 4, -1] {
        assert_eq!(
            traced_function_text_from_parts(&tag.to_le_bytes(), &padded),
            None,
            "tag {tag}"
        );
    }
    assert_eq!(traced_function_text_from_parts(&[2, 0], &padded), None);
}

#[test]
fn the_printed_text_is_read_from_the_first_native_argument() {
    let mut block = vec![0_u8; debug_print_args_len()];
    let slot = 0x10 + DEBUG_PRINT_TEXT_ARG_INDEX * 0x90 + 8;
    block[slot..slot + 4].copy_from_slice(&2_i32.to_le_bytes());
    let text = b"A_STONE_FIELD_02->EID_0200_a0030g_Talk02";
    block[slot + 0x10..slot + 0x10 + text.len()].copy_from_slice(text);
    assert_eq!(
        traced_function_text(&block).as_deref(),
        Some("A_STONE_FIELD_02->EID_0200_a0030g_Talk02")
    );
    assert_eq!(traced_function_text(&[0_u8; 8]), None);
}

#[test]
fn the_table_is_sorted_and_every_callee_is_a_row() {
    for pair in SCRIPT_FUNCTIONS.windows(2) {
        assert!((pair[0].0, pair[0].1) < (pair[1].0, pair[1].1), "{pair:?}");
    }
    for (phase, name, _, _, callees) in SCRIPT_FUNCTIONS.iter() {
        for callee in callees.iter() {
            assert!(
                script_function(phase, callee).is_some(),
                "{phase} {name} -> {callee}"
            );
        }
    }
}

#[test]
fn every_shared_event_base_has_a_row_of_its_family() {
    for (phase, base) in SHARED_EVENT_BASES.iter() {
        let has_row = SCRIPT_FUNCTIONS.iter().any(|(table_phase, name, ..)| {
            table_phase == phase
                && (name == base
                    || name
                        .strip_prefix(base)
                        .is_some_and(|rest| rest.starts_with('_')))
        });
        assert!(has_row, "{phase} {base}");
    }
}

#[test]
fn a_talk_that_starts_a_scene_closes_the_menu_though_it_is_not_relayed_by_name() {
    for name in [
        "EID_0200_a0040g_Talk",
        "AttackCount_Talk",
        "EID_0200_a0040g_Continue",
    ] {
        assert!(shows_modal_ui(LABEL, name), "{name}");
        assert!(script_function(LABEL, name).is_none(), "{name}");
    }
}

#[test]
fn the_modal_ui_table_is_sorted_and_holds_no_relayed_row() {
    for pair in MODAL_UI_FUNCTIONS.windows(2) {
        assert!(pair[0] < pair[1], "{pair:?}");
    }
    for (phase, name) in MODAL_UI_FUNCTIONS.iter() {
        assert!(script_function(phase, name).is_none(), "{phase} {name}");
    }
}

#[test]
fn the_bomb_callbacks_of_the_bomb_floor_are_plain_steps() {
    for name in [
        "Bomb_Tutorial01",
        "Bomb_Tutorial02",
        "Obj_Bomb_Flag01",
        "Obj_Bomb_Flag02",
    ] {
        let function = replicant_game::script_functions::script_function("A_MOUNT_ROBOT_02", name)
            .unwrap_or_else(|| panic!("{name} has no row"));
        assert!(!function.starts_scene && !function.shows_modal_ui, "{name}");
    }
}

#[test]
fn only_the_cart_puts_are_shared_and_each_game_keeps_its_own_cart_callbacks() {
    for pack in [
        "A_MOUNT_ROBOT_02",
        "A_MOUNT_ROBOT_03",
        "B_MOUNT_ROBOT_02",
        "B_MOUNT_ROBOT_03",
    ] {
        assert!(script_function(pack, "Put_Truck_01").is_some(), "{pack}");
        assert!(script_function(pack, "Put_Truck_02").is_some(), "{pack}");
        for own in [
            "ChangeCam_Truck01",
            "ChangeCam_Truck02",
            "Put_Truck_03",
            "Put_Truck_04",
            "SET_FALL_TRUCK_01",
            "SET_FALL_TRUCK_02",
            "StartTruck_CameraSetting",
            "Visible_Control",
        ] {
            assert!(script_function(pack, own).is_none(), "{pack} {own}");
        }
    }
}
