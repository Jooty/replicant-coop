use replicant_game::callback_gate::{
    EndQueue, GateRole, ParkedCall, ParkedKind, QueueEvent, Released, is_end_callback,
};
use replicant_game::message_handlers::{
    HandlerArrival, HandlerArrivalFacts, HandlerDrop, HandlerFire, HandlerFireFacts,
    HandlerRegistered, MessageHandlers, PLAYER_LOCAL_HANDLER_NAMES, PLAYER_LOCAL_MESSAGE_IDS,
    RELAYED_HANDLER_PASS_MS, RelayedHandlerPasses, SET_MESSAGE_CALLBACK_MESSAGE_ID_ARG_INDEX,
    handler_arrival, handler_fire, handler_name_from_parts, message_id_from_parts,
    message_id_tag_range, message_id_value_range, message_name_tag_range, message_name_text_range,
};
use replicant_game::script_functions::{
    ClearCause, DrainStep, FunctionArrival, FunctionArrivalFacts, FunctionDrop, FunctionsRan,
    LateCopy, PhaseLoadWatch, function_arrival, is_restart_function, late_copy_verdict,
    plan_forced_drain, script_function,
};
use replicant_proto::StoryCall;

const LABEL: &str = "A_STONE_FIELD_02";
const PHASE: u16 = 7;
const KILL_COUNTER_END: &str = "EID_0200_a0037g_02_End";
const GUARD_CHANGE: &str = "Talk_Text_01_GUARD_CHANGE";
const BOSS_STAGE_END: &str = "CB_BossCA_01";
const THIRD_STAGE_END: &str = "CB_BossCA_03";
const THIRD_STAGE_CINEMATIC_END: &str = "EID_0200_a0075g_End";
const GAME_OVER: &str = "LIB_GameOver";
const WAITING_TALK_END: &str = "EID_0200_a0030g_Talk04_End";
const LATE_COPY: &str = "EID_0200_a0050_Start";

const RUN32_REGISTRATIONS: [(&str, i32); 16] = [
    (KILL_COUNTER_END, 212),
    ("Talk_Text_02", 226),
    (GUARD_CHANGE, 244),
    ("Talk_Text_01_CHARGE_SHOUT", 245),
    ("Talk_Text_01_CHARGE", 247),
    ("Talk_Text_01_Damage", 213),
    ("First_CA_Generation", 299),
    ("Talk_Text_01_CA_END", 231),
    (BOSS_STAGE_END, 241),
    (THIRD_STAGE_END, 241),
    ("Talk_Text_03_SHOT", 246),
    ("Talk_Text_02_Damage", 213),
    ("B_Lance_Talk", 222),
    ("CA_Generation_02", 299),
    (GAME_OVER, 210),
    ("SQ_Player_Receive_Damage", 470),
];

const RUN32_HOST_ORDER: [&str; 15] = [
    KILL_COUNTER_END,
    KILL_COUNTER_END,
    KILL_COUNTER_END,
    "Talk_Text_02",
    GUARD_CHANGE,
    "Talk_Text_01_CHARGE_SHOUT",
    "Talk_Text_01_CHARGE",
    "Talk_Text_01_Damage",
    "First_CA_Generation",
    "Talk_Text_01_CA_END",
    BOSS_STAGE_END,
    "Talk_Text_03_SHOT",
    "Talk_Text_02_Damage",
    "B_Lance_Talk",
    "CA_Generation_02",
];

fn registry() -> MessageHandlers {
    let mut handlers = MessageHandlers::new();
    for (name, id) in RUN32_REGISTRATIONS {
        assert_eq!(handlers.register(PHASE, name, id), HandlerRegistered::New);
    }
    handlers
}

fn call(name: &str) -> StoryCall {
    StoryCall {
        phase: PHASE,
        pack: 0,
        function: name.to_owned(),
        args: Vec::new(),
    }
}

fn arrival_facts(queue_is_empty: bool) -> HandlerArrivalFacts<'static> {
    HandlerArrivalFacts {
        enabled: true,
        local_is_guest: true,
        local_in_play: true,
        local_phase: Some(PHASE),
        call_phase: PHASE,
        phase_label: LABEL,
        queue_is_empty,
    }
}

fn fire_facts(handlers: &MessageHandlers, role: GateRole, name: &str) -> HandlerFireFacts {
    HandlerFireFacts {
        host_driven_handler: handlers.is_host_driven(PHASE, name),
        role: Some(role),
        both_in_phase: true,
    }
}

struct Guest {
    handlers: MessageHandlers,
    queue: EndQueue,
    passes: RelayedHandlerPasses,
    ran: FunctionsRan,
    order: Vec<String>,
    held: Vec<String>,
    own_runs: Vec<String>,
}

impl Guest {
    fn new() -> Self {
        Self {
            handlers: registry(),
            queue: EndQueue::new(),
            passes: RelayedHandlerPasses::new(),
            ran: FunctionsRan::new(),
            order: Vec::new(),
            held: Vec::new(),
            own_runs: Vec::new(),
        }
    }

    fn own_fire(&mut self, name: &str, now: u64) -> HandlerFire {
        let facts = fire_facts(&self.handlers, GateRole::Guest, name);
        let fire = handler_fire(&facts, &mut self.passes, name, now);
        match fire {
            HandlerFire::GuestHoldsForTheHost => self.held.push(name.to_owned()),
            HandlerFire::NotAHandler | HandlerFire::RunHereOnly => {
                self.own_runs.push(name.to_owned());
            }
            HandlerFire::GuestRunsTheHostsCall | HandlerFire::HostRunsAndSends => {}
        }
        fire
    }

    fn run_handler(&mut self, name: &str, now: u64) {
        self.ran.note(PHASE, name);
        self.passes.expect_run(name, now);
        assert_eq!(self.own_fire(name, now), HandlerFire::GuestRunsTheHostsCall);
        self.order.push(name.to_owned());
    }

    fn run_end(&mut self, name: &str) {
        self.order.push(name.to_owned());
    }

    fn run_released(&mut self, released: Released, now: u64) {
        match released {
            Released::End { end, .. } => self.run_end(&end.name),
            Released::Call { parked, .. } => self.run_handler(&parked.call.function, now),
        }
    }

    fn host_ran(&mut self, name: &str, now: u64) {
        let facts = HandlerArrivalFacts {
            queue_is_empty: self.queue.is_empty(),
            ..arrival_facts(true)
        };
        let parked = ParkedCall {
            kind: ParkedKind::Handler,
            call: call(name),
        };
        match handler_arrival(&facts, name) {
            HandlerArrival::Drop(reason) => panic!("dropped: {}", reason.describe()),
            HandlerArrival::RunNow => self.run_handler(name, now),
            HandlerArrival::Park => {
                self.queue.park_call_always(parked, now);
            }
            HandlerArrival::ForcedDrain => {
                self.queue.park_call_always(parked, now);
                let taken = self.queue.take_through_handler(name);
                for step in plan_forced_drain(taken, name, LABEL) {
                    match step {
                        DrainStep::Run(released) => self.run_released(released, now),
                        DrainStep::Superseded { .. } => panic!("a handler is never superseded"),
                    }
                }
            }
        }
    }

    fn tick(&mut self, now: u64) {
        let events = self
            .queue
            .advance(now, |_end| replicant_game::callback_gate::EndFollow::Mirror);
        for event in events {
            match event {
                QueueEvent::Run { end, .. } => self.run_end(&end.name),
                QueueEvent::RunCall { parked, .. } => {
                    self.run_handler(&parked.call.function, now);
                }
                QueueEvent::Dropped { .. } | QueueEvent::StillWaiting { .. } => {}
            }
        }
    }
}

#[test]
fn the_exclusion_list_keeps_game_over_and_the_players_own_messages_local() {
    let handlers = registry();
    assert!(!handlers.is_host_driven(PHASE, GAME_OVER));
    assert!(!handlers.is_host_driven(PHASE, "SQ_Player_Receive_Damage"));
    assert!(handlers.is_host_driven(PHASE, KILL_COUNTER_END));
    assert!(handlers.is_host_driven(PHASE, BOSS_STAGE_END));
    assert!(handlers.is_registered(PHASE, GAME_OVER));
    assert!(!handlers.is_host_driven(PHASE + 1, KILL_COUNTER_END));
    assert!(PLAYER_LOCAL_MESSAGE_IDS.contains(&210));
    assert!(PLAYER_LOCAL_HANDLER_NAMES.contains(&GAME_OVER));
}

#[test]
fn a_handler_of_a_player_local_message_is_player_local_and_a_fight_can_add_messages() {
    let mut handlers = registry();
    assert!(handlers.is_player_local(PHASE, GAME_OVER, &[]));
    assert!(!handlers.is_player_local(PHASE, KILL_COUNTER_END, &[]));
    assert!(handlers.is_player_local(PHASE, KILL_COUNTER_END, &[212]));
    assert!(!handlers.is_player_local(PHASE + 1, GAME_OVER, &[]));
    handlers.register(PHASE, "Talk_Radio_Own", 470);
    assert!(handlers.is_player_local(PHASE, "Talk_Radio_Own", &[]));
}

#[test]
fn a_handler_registered_twice_is_known_and_the_table_does_not_grow() {
    let mut handlers = registry();
    assert_eq!(
        handlers.register(PHASE, KILL_COUNTER_END, 212),
        HandlerRegistered::Known
    );
}

#[test]
fn run32_the_guest_runs_the_hosts_handlers_once_in_order_and_holds_its_own_fires() {
    let mut guest = Guest::new();
    guest.queue.push(WAITING_TALK_END, PHASE, 0);
    let mut now = 1_000;
    for (index, name) in RUN32_HOST_ORDER.iter().enumerate() {
        guest.host_ran(name, now);
        if index == 4 {
            assert_eq!(
                guest.own_fire(GUARD_CHANGE, now),
                HandlerFire::GuestHoldsForTheHost
            );
        }
        now += 500;
        guest.tick(now);
    }
    let mut expected = vec![WAITING_TALK_END.to_owned()];
    expected.extend(RUN32_HOST_ORDER.iter().map(|name| (*name).to_owned()));
    assert_eq!(guest.order, expected);
    assert_eq!(guest.held, [GUARD_CHANGE]);
    assert!(guest.own_runs.is_empty());
    assert!(guest.queue.is_empty());
}

#[test]
fn run32_the_guests_own_game_over_is_never_held() {
    let mut guest = Guest::new();
    assert_eq!(guest.own_fire(GAME_OVER, 100), HandlerFire::NotAHandler);
    assert_eq!(guest.own_runs, [GAME_OVER]);
    assert!(guest.held.is_empty());
}

#[test]
fn run32_the_hosts_handlers_run_and_send_and_the_hosts_game_over_stays_its_own() {
    let handlers = registry();
    let mut passes = RelayedHandlerPasses::new();
    for name in RUN32_HOST_ORDER {
        let facts = fire_facts(&handlers, GateRole::Host, name);
        assert_eq!(
            handler_fire(&facts, &mut passes, name, 0),
            HandlerFire::HostRunsAndSends
        );
    }
    let facts = fire_facts(&handlers, GateRole::Host, GAME_OVER);
    assert_eq!(
        handler_fire(&facts, &mut passes, GAME_OVER, 0),
        HandlerFire::NotAHandler
    );
}

#[test]
fn when_the_partner_is_not_in_this_phase_each_game_runs_its_own() {
    let handlers = registry();
    let mut passes = RelayedHandlerPasses::new();
    for role in [GateRole::Host, GateRole::Guest] {
        let facts = HandlerFireFacts {
            both_in_phase: false,
            ..fire_facts(&handlers, role, GUARD_CHANGE)
        };
        assert_eq!(
            handler_fire(&facts, &mut passes, GUARD_CHANGE, 0),
            HandlerFire::RunHereOnly
        );
    }
    let alone = HandlerFireFacts {
        role: None,
        ..fire_facts(&handlers, GateRole::Guest, GUARD_CHANGE)
    };
    assert_eq!(
        handler_fire(&alone, &mut passes, GUARD_CHANGE, 0),
        HandlerFire::RunHereOnly
    );
}

#[test]
fn a_relayed_run_lets_exactly_one_pop_through_and_the_pass_expires() {
    let mut passes = RelayedHandlerPasses::new();
    passes.expect_run(GUARD_CHANGE, 0);
    assert!(passes.take(GUARD_CHANGE, 10));
    assert!(!passes.take(GUARD_CHANGE, 20));
    passes.expect_run(GUARD_CHANGE, 100);
    assert!(!passes.take(GUARD_CHANGE, 100 + RELAYED_HANDLER_PASS_MS + 1));
}

#[test]
fn a_handler_arrival_is_followed_only_by_the_guest_in_the_same_phase() {
    let mut facts = arrival_facts(true);
    assert_eq!(
        handler_arrival(&facts, GUARD_CHANGE),
        HandlerArrival::RunNow
    );
    assert_eq!(
        handler_arrival(&arrival_facts(false), GUARD_CHANGE),
        HandlerArrival::Park
    );
    assert_eq!(
        handler_arrival(&arrival_facts(false), KILL_COUNTER_END),
        HandlerArrival::ForcedDrain
    );
    facts.local_phase = Some(PHASE + 1);
    assert_eq!(
        handler_arrival(&facts, GUARD_CHANGE),
        HandlerArrival::Drop(HandlerDrop::WrongPhase)
    );
    facts.local_phase = Some(PHASE);
    facts.local_is_guest = false;
    assert_eq!(
        handler_arrival(&facts, GUARD_CHANGE),
        HandlerArrival::Drop(HandlerDrop::NotTheGuest)
    );
}

#[test]
fn the_table_covers_the_run32_handlers_so_scenes_and_menus_are_known() {
    for (name, _) in RUN32_REGISTRATIONS {
        if name == GAME_OVER || name.starts_with("SQ_") {
            continue;
        }
        assert!(script_function(LABEL, name).is_some(), "{name} missing");
    }
    assert!(
        script_function(LABEL, KILL_COUNTER_END)
            .unwrap()
            .starts_scene
    );
    assert!(script_function(LABEL, BOSS_STAGE_END).unwrap().starts_scene);
    assert!(!script_function(LABEL, GUARD_CHANGE).unwrap().shows_modal_ui);
}

fn function_facts() -> FunctionArrivalFacts<'static> {
    FunctionArrivalFacts {
        enabled: true,
        local_in_play: true,
        local_phase: Some(PHASE),
        call_phase: PHASE,
        phase_label: LABEL,
        queue_is_empty: true,
    }
}

#[test]
fn a_function_the_host_traced_inside_a_handler_is_dropped_when_the_handler_ran_here() {
    let mut guest = Guest::new();
    guest.run_handler(THIRD_STAGE_END, 0);
    let verdict = function_arrival(
        &function_facts(),
        THIRD_STAGE_CINEMATIC_END,
        &guest.ran,
        &[],
    );
    assert_eq!(
        verdict,
        FunctionArrival::Drop(FunctionDrop::CalleeOf(THIRD_STAGE_END.to_owned()))
    );
}

#[test]
fn a_function_the_host_traced_inside_a_handler_is_dropped_while_the_handler_is_queued() {
    let mut guest = Guest::new();
    guest.queue.push(WAITING_TALK_END, PHASE, 0);
    guest.host_ran(GUARD_CHANGE, 10);
    guest.host_ran("Talk_Text_02", 20);
    let facts = HandlerArrivalFacts {
        queue_is_empty: false,
        ..arrival_facts(false)
    };
    assert_eq!(
        handler_arrival(&facts, THIRD_STAGE_END),
        HandlerArrival::ForcedDrain
    );
    guest.queue.park_call_always(
        ParkedCall {
            kind: ParkedKind::Handler,
            call: call(THIRD_STAGE_END),
        },
        30,
    );
    let queued = guest.queue.queued_function_names(PHASE);
    let verdict = function_arrival(
        &function_facts(),
        THIRD_STAGE_CINEMATIC_END,
        &guest.ran,
        &queued,
    );
    assert_eq!(
        verdict,
        FunctionArrival::Drop(FunctionDrop::CalleeOf(THIRD_STAGE_END.to_owned()))
    );
}

#[test]
fn run32_a_late_copy_of_a_step_this_game_ran_nested_is_not_started_again() {
    let mut ran = FunctionsRan::new();
    assert_eq!(late_copy_verdict(&ran, PHASE, LATE_COPY), LateCopy::RunIt);
    ran.note(PHASE, LATE_COPY);
    assert_eq!(
        late_copy_verdict(&ran, PHASE, LATE_COPY),
        LateCopy::RanHereTraced
    );
    assert_eq!(
        late_copy_verdict(&ran, PHASE + 1, LATE_COPY),
        LateCopy::RunIt
    );
    assert!(script_function(LABEL, LATE_COPY).is_some());
}

#[test]
fn the_end_named_handler_is_still_an_end_callback() {
    assert!(is_end_callback(KILL_COUNTER_END));
    assert!(!is_end_callback(GUARD_CHANGE));
}

#[test]
fn the_registration_arguments_are_read_from_the_fourth_and_first_slots() {
    assert_eq!(SET_MESSAGE_CALLBACK_MESSAGE_ID_ARG_INDEX, 3);
    let name_tag = 2_i32.to_le_bytes();
    let mut text = vec![0_u8; message_name_text_range().len()];
    text[..GUARD_CHANGE.len()].copy_from_slice(GUARD_CHANGE.as_bytes());
    assert_eq!(
        handler_name_from_parts(&name_tag, &text).as_deref(),
        Some(GUARD_CHANGE)
    );
    assert_eq!(handler_name_from_parts(&4_i32.to_le_bytes(), &text), None);
    assert_eq!(message_name_tag_range().len(), 4);

    let mut value = vec![0_u8; message_id_value_range().len()];
    value[..4].copy_from_slice(&244_i32.to_le_bytes());
    assert_eq!(
        message_id_from_parts(&4_i32.to_le_bytes(), &value),
        Some(244)
    );
    value.copy_from_slice(&212.0_f64.to_le_bytes());
    assert_eq!(
        message_id_from_parts(&1_i32.to_le_bytes(), &value),
        Some(212)
    );
    value.copy_from_slice(&212.5_f64.to_le_bytes());
    assert_eq!(message_id_from_parts(&1_i32.to_le_bytes(), &value), None);
    assert_eq!(message_id_from_parts(&2_i32.to_le_bytes(), &value), None);
    assert!(message_id_tag_range().start > message_name_tag_range().start);
}

#[test]
fn run32_second_a0050_start_is_dropped_without_a_restart_and_runs_after_one() {
    let mut watch = PhaseLoadWatch::new();
    let mut ran = FunctionsRan::new();
    assert_eq!(watch.clear_cause(1, 0), Some(ClearCause::PhaseLoad));
    ran.note(PHASE, LATE_COPY);
    assert_eq!(watch.clear_cause(1, 0), None);
    assert_eq!(
        late_copy_verdict(&ran, PHASE, LATE_COPY),
        LateCopy::RanHereTraced
    );
    assert_eq!(watch.clear_cause(1, 1), Some(ClearCause::RestartSignal));
    ran.reset();
    assert_eq!(late_copy_verdict(&ran, PHASE, LATE_COPY), LateCopy::RunIt);
}

#[test]
fn game_over_and_continue_functions_are_restart_signals() {
    assert!(is_restart_function("LIB_GameOver"));
    assert!(is_restart_function("a0050_Continue"));
    assert!(!is_restart_function("EID_0200_a0050_Start"));
    assert!(!is_restart_function("Continue_Text"));
}

#[test]
fn message_215_is_player_local() {
    assert!(PLAYER_LOCAL_MESSAGE_IDS.contains(&215));
}

#[test]
fn the_cart_boarding_handler_runs_on_each_players_own_game() {
    let mut handlers = MessageHandlers::new();
    assert_eq!(
        handlers.register(PHASE, "ChangeCam_Truck02", 239),
        HandlerRegistered::New
    );
    assert!(!handlers.is_host_driven(PHASE, "ChangeCam_Truck02"));
    assert!(handlers.is_player_local(PHASE, "ChangeCam_Truck02", &[]));
}
