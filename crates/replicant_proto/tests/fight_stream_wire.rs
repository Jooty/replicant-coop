use replicant_proto::{
    FightPromptCount, FightRadioWaitEnded, FightRequest, FightRequestAnswer, FightRequestKind,
    FightRequestOutcome, FightRequestRefusal, FightStreamEntry, FightTape, FightTapeValue,
    FightWindowChange, FightWindowMark, MAX_FIGHT_PROMPT_NATIVE_CODE, MAX_FIGHT_PROMPT_PAGES,
    MAX_FIGHT_RADIO_WAIT_INDEX, MAX_FIGHT_TAPE_CALL_INDEX, MAX_MESSAGE_BYTES, Message,
    PROTOCOL_VERSION, StoryCallArg, WireError, decode, encode,
};

fn entry() -> FightStreamEntry {
    FightStreamEntry {
        phase: 23,
        epoch: 2,
        seq: 41,
        name: "EID_0200_a0037g_02_End".to_owned(),
        args: vec![
            StoryCallArg::Int(212),
            StoryCallArg::Number(7),
            StoryCallArg::Text(b"count_kill_enemy".to_vec()),
        ],
    }
}

fn mark(change: FightWindowChange) -> FightWindowMark {
    FightWindowMark {
        phase: 23,
        epoch: 2,
        number: 1,
        part: 1,
        change,
    }
}

#[test]
fn a_fight_stream_entry_round_trips_fits_a_message_and_is_relayed() {
    let message = Message::FightStream(entry());
    assert!(message.is_relayed());
    assert_eq!(message.validate(), Ok(()));
    let bytes = encode(&message).unwrap();
    assert!(bytes.len() <= MAX_MESSAGE_BYTES);
    assert_eq!(decode(&bytes), Ok(message));
    assert_eq!(PROTOCOL_VERSION, 67);
}

#[test]
fn a_fight_stream_entry_with_the_largest_arguments_fits_a_message() {
    let message = Message::FightStream(FightStreamEntry {
        seq: u64::MAX,
        epoch: u32::MAX,
        name: "x".repeat(63),
        args: vec![StoryCallArg::Text(vec![b'a'; 48]); 6],
        ..entry()
    });
    assert_eq!(message.validate(), Ok(()));
    assert!(encode(&message).unwrap().len() <= MAX_MESSAGE_BYTES);
}

#[test]
fn a_fight_stream_entry_with_a_bad_name_or_arguments_is_refused() {
    for name in [String::new(), "two words".to_owned(), "x".repeat(64)] {
        let bad = FightStreamEntry { name, ..entry() };
        assert_eq!(
            bad.validate(),
            Err(WireError::OutOfRange { field: "function" })
        );
    }
    let too_many = FightStreamEntry {
        args: vec![StoryCallArg::Int(0); 7],
        ..entry()
    };
    assert_eq!(
        too_many.validate(),
        Err(WireError::OutOfRange { field: "args" })
    );
}

#[test]
fn a_fight_window_mark_round_trips_and_is_relayed_for_open_close_and_entry_end() {
    for change in [
        FightWindowChange::Opened { armed_by: None },
        FightWindowChange::Opened {
            armed_by: Some("EID_0200_a0030g_Talk01".to_owned()),
        },
        FightWindowChange::Restarted,
        FightWindowChange::Closed { end_seq: 120 },
        FightWindowChange::EntryEnded { seq: u64::MAX },
    ] {
        let message = Message::FightWindow(mark(change));
        assert!(message.is_relayed());
        assert_eq!(message.validate(), Ok(()));
        assert_eq!(decode(&encode(&message).unwrap()), Ok(message));
    }
}

#[test]
fn a_fight_window_mark_without_a_fight_number_is_refused() {
    let bad = FightWindowMark {
        number: 0,
        ..mark(FightWindowChange::Opened { armed_by: None })
    };
    assert_eq!(
        bad.validate(),
        Err(WireError::OutOfRange {
            field: "fight window"
        })
    );
}

#[test]
fn a_fight_request_for_a_pop_or_a_restart_round_trips_fits_a_message_and_is_relayed() {
    for kind in [
        FightRequestKind::RunPop {
            name: "x".repeat(63),
            args: vec![StoryCallArg::Text(vec![b'a'; 48]); 6],
            copy_number: u32::MAX,
        },
        FightRequestKind::RestartPhase,
    ] {
        let message = Message::FightRequest(FightRequest {
            phase: 23,
            epoch: u32::MAX,
            kind,
        });
        assert!(message.is_relayed());
        assert_eq!(message.validate(), Ok(()));
        let bytes = encode(&message).unwrap();
        assert!(bytes.len() <= MAX_MESSAGE_BYTES);
        assert_eq!(decode(&bytes), Ok(message));
    }
}

#[test]
fn a_fight_request_with_a_bad_name_is_refused() {
    let bad = FightRequest {
        phase: 23,
        epoch: 2,
        kind: FightRequestKind::RunPop {
            name: "two words".to_owned(),
            args: Vec::new(),
            copy_number: 1,
        },
    };
    assert_eq!(
        bad.validate(),
        Err(WireError::OutOfRange { field: "function" })
    );
}

#[test]
fn every_answer_to_a_fight_request_round_trips_fits_a_message_and_is_relayed() {
    let refusals = [
        FightRequestRefusal::NotAWindowFunction,
        FightRequestRefusal::AlreadyStreamed,
        FightRequestRefusal::Full,
        FightRequestRefusal::GaveUp,
        FightRequestRefusal::FightMovedOn,
    ];
    let outcomes = std::iter::once(FightRequestOutcome::Ran)
        .chain(refusals.map(|reason| FightRequestOutcome::Refused { reason }));
    for outcome in outcomes {
        let message = Message::FightRequestAnswer(FightRequestAnswer {
            phase: 23,
            epoch: u32::MAX,
            name: "x".repeat(63),
            copy_number: u32::MAX,
            outcome,
        });
        assert!(message.is_relayed());
        assert_eq!(message.validate(), Ok(()));
        let bytes = encode(&message).unwrap();
        assert!(bytes.len() <= MAX_MESSAGE_BYTES);
        assert_eq!(decode(&bytes), Ok(message));
    }
    let bad = FightRequestAnswer {
        phase: 23,
        epoch: 2,
        name: "two words".to_owned(),
        copy_number: 1,
        outcome: FightRequestOutcome::Ran,
    };
    assert_eq!(
        bad.validate(),
        Err(WireError::OutOfRange { field: "function" })
    );
}

fn tape(value: FightTapeValue) -> FightTape {
    FightTape {
        phase: 23,
        epoch: 2,
        seq: 41,
        native: "_GetActorLife".to_owned(),
        call_index: 3,
        value,
    }
}

#[test]
fn a_fight_tape_value_of_each_kind_round_trips_fits_a_message_and_is_relayed() {
    for value in [
        FightTapeValue::Int(-7),
        FightTapeValue::Number(0.375),
        FightTapeValue::Missing,
    ] {
        let message = Message::FightTape(FightTape {
            seq: u64::MAX,
            epoch: u32::MAX,
            native: "x".repeat(63),
            call_index: MAX_FIGHT_TAPE_CALL_INDEX,
            ..tape(value)
        });
        assert!(message.is_relayed());
        assert_eq!(message.validate(), Ok(()));
        let bytes = encode(&message).unwrap();
        assert!(bytes.len() <= MAX_MESSAGE_BYTES);
        assert_eq!(decode(&bytes), Ok(message));
    }
    assert_eq!(PROTOCOL_VERSION, 67);
}

#[test]
fn a_fight_tape_with_a_bad_native_call_index_or_number_is_refused() {
    let bad_name = FightTape {
        native: "two words".to_owned(),
        ..tape(FightTapeValue::Int(1))
    };
    assert_eq!(
        bad_name.validate(),
        Err(WireError::OutOfRange { field: "function" })
    );
    let bad_index = FightTape {
        call_index: MAX_FIGHT_TAPE_CALL_INDEX + 1,
        ..tape(FightTapeValue::Int(1))
    };
    assert_eq!(
        bad_index.validate(),
        Err(WireError::OutOfRange {
            field: "call index"
        })
    );
    for number in [f64::NAN, f64::INFINITY] {
        assert_eq!(
            tape(FightTapeValue::Number(number)).validate(),
            Err(WireError::OutOfRange { field: "value" })
        );
    }
}

#[test]
fn an_opening_armed_by_a_name_no_function_could_have_is_refused() {
    let bad = mark(FightWindowChange::Opened {
        armed_by: Some("two words".to_owned()),
    });
    assert_eq!(
        bad.validate(),
        Err(WireError::OutOfRange { field: "function" })
    );
}

#[test]
fn a_radio_wait_end_round_trips_is_relayed_and_refuses_an_index_past_the_limit() {
    let ended = FightRadioWaitEnded {
        phase: 23,
        epoch: u32::MAX,
        seq: u64::MAX,
        wait_index: MAX_FIGHT_RADIO_WAIT_INDEX,
        host_waited_ms: u32::MAX,
    };
    let message = Message::FightRadioWait(ended);
    assert!(message.is_relayed());
    assert_eq!(message.validate(), Ok(()));
    let bytes = encode(&message).unwrap();
    assert!(bytes.len() <= MAX_MESSAGE_BYTES);
    assert_eq!(decode(&bytes), Ok(message));
    let too_far = FightRadioWaitEnded {
        wait_index: MAX_FIGHT_RADIO_WAIT_INDEX + 1,
        ..ended
    };
    assert_eq!(
        Message::FightRadioWait(too_far).validate(),
        Err(WireError::OutOfRange {
            field: "radio wait index"
        })
    );
    assert_eq!(PROTOCOL_VERSION, 67);
}

#[test]
fn a_prompt_count_round_trips_is_relayed_and_refuses_a_count_or_native_past_the_limit() {
    let count = FightPromptCount {
        phase: 23,
        kind: 0x9C5,
        id: 2001,
        page: MAX_FIGHT_PROMPT_PAGES,
        dismissed: MAX_FIGHT_PROMPT_PAGES,
        native: MAX_FIGHT_PROMPT_NATIVE_CODE,
        in_fight: false,
    };
    let message = Message::FightPromptCount(count);
    assert!(message.is_relayed());
    assert_eq!(message.validate(), Ok(()));
    let bytes = encode(&message).unwrap();
    assert!(bytes.len() <= MAX_MESSAGE_BYTES);
    assert_eq!(decode(&bytes), Ok(message));
    let too_many = FightPromptCount {
        dismissed: MAX_FIGHT_PROMPT_PAGES + 1,
        ..count
    };
    let too_far = FightPromptCount {
        page: MAX_FIGHT_PROMPT_PAGES + 1,
        ..count
    };
    let unknown_native = FightPromptCount {
        native: MAX_FIGHT_PROMPT_NATIVE_CODE + 1,
        ..count
    };
    assert_eq!(
        Message::FightPromptCount(unknown_native).validate(),
        Err(WireError::OutOfRange {
            field: "prompt native"
        })
    );
    assert!(Message::FightPromptCount(too_far).validate().is_err());
    assert_eq!(
        Message::FightPromptCount(too_many).validate(),
        Err(WireError::OutOfRange {
            field: "prompt pages"
        })
    );
    assert_eq!(PROTOCOL_VERSION, 67);
}
