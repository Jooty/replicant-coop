use std::time::Duration;

use nc_launcher_core::{
    CONSOLE_LINE_TYPING_MS, CONSOLE_SHORTEST_RUN_MS, ConsoleClock, ConsoleGameFacts,
    ConsoleLineKind, ConsoleSave, ConsoleScript, ConsoleView, HostingFacts, JoiningFacts, NetState,
    StepEnd, TimedProgress, hosting_script, joining_script, judge_rejection_text,
};
use nc_net::{LinkProgress, PathRoute};
use replicant_proto::RejectReason;

const TICKET: &str = "rcn1fixtureticketlongerthantwelve";
const FOLDER: &str = "~/games/fixture";
const PLAYER: &str = "Jay";
const HOST: &str = "Mira";

fn game(waiting_update: Option<&'static str>) -> ConsoleGameFacts<'static> {
    ConsoleGameFacts {
        protocol: 67,
        installed_build: Some("3f2a91c"),
        waiting_update,
        player_name: PLAYER,
    }
}

fn hosting_with(
    answer: Option<Result<&str, &str>>,
    state: &NetState,
    saved_game: Option<Option<ConsoleSave<'static>>>,
    executable: Option<&'static str>,
) -> ConsoleScript {
    hosting_told(answer, state, saved_game, executable, &[])
}

fn hosting_told(
    answer: Option<Result<&str, &str>>,
    state: &NetState,
    saved_game: Option<Option<ConsoleSave<'static>>>,
    executable: Option<&'static str>,
    progress: &[TimedProgress],
) -> ConsoleScript {
    hosting_script(HostingFacts {
        game_folder: Ok(FOLDER),
        executable,
        mod_files: Ok(()),
        hook_file: "hook.dll",
        game: game(None),
        saved_game,
        seats: 2,
        enemy_health_percent: 60,
        enemy_damage_percent: 10,
        host_answer: answer,
        state,
        progress,
    })
}

fn hosting(answer: Option<Result<&str, &str>>, state: &NetState) -> ConsoleScript {
    hosting_with(answer, state, Some(None), Some("game.exe"))
}

fn hosted() -> NetState {
    NetState::Hosting {
        ticket: TICKET.to_owned(),
    }
}

fn joining(
    join_answer: Option<Result<(), &str>>,
    state: &NetState,
    rejected: Option<RejectReason>,
    progress: &[TimedProgress],
) -> ConsoleScript {
    joining_script(JoiningFacts {
        ticket_text: TICKET,
        ticket: Ok(()),
        game: game(None),
        join_answer,
        state,
        rejected,
        progress,
        own_slot: Some(1),
    })
}

fn at(at_ms: f64, progress: LinkProgress) -> TimedProgress {
    TimedProgress { at_ms, progress }
}

fn host_answered_at(ms: f64) -> Vec<TimedProgress> {
    vec![
        at(
            ms - 300.0,
            LinkProgress::TicketRead {
                host_id: "9f3c2a71e0".to_owned(),
                relay: Some("euc1-1".to_owned()),
                direct_addresses: 2,
            },
        ),
        at(
            ms,
            LinkProgress::HostAnswered {
                after: Duration::from_millis(300),
            },
        ),
        at(ms + 1.0, LinkProgress::PathTried(PathRoute::Relayed)),
        at(
            ms + 1.0,
            LinkProgress::PathWon {
                route: PathRoute::Relayed,
                rtt: Some(Duration::from_millis(62)),
            },
        ),
        at(ms + 2.0, LinkProgress::HelloSent { protocol: 67 }),
    ]
}

fn welcomed(mut progress: Vec<TimedProgress>, ms: f64) -> Vec<TimedProgress> {
    progress.push(at(
        ms,
        LinkProgress::WelcomeRead {
            seat: 1,
            host_name: HOST.to_owned(),
        },
    ));
    progress
}

fn connected() -> NetState {
    NetState::Connected {
        peer_name: HOST.to_owned(),
    }
}

fn shown_labels(view: &ConsoleView) -> Vec<&'static str> {
    view.lines.iter().map(|line| line.label).collect()
}

fn view_as_it_came(
    script_at: impl Fn(f64) -> ConsoleScript,
    frames_ms: &[f64],
) -> (ConsoleScript, ConsoleView) {
    let mut clock = ConsoleClock::new(0.0);
    let mut last = None;
    for now in frames_ms {
        let script = script_at(*now);
        clock.observe(&script, *now);
        let view = clock.view(&script, *now, true);
        last = Some((script, view));
    }
    last.expect("at least one frame")
}

fn ends(script: &ConsoleScript) -> Vec<StepEnd> {
    script.steps.iter().map(|step| step.end).collect()
}

fn labels(script: &ConsoleScript) -> Vec<&'static str> {
    script
        .steps
        .iter()
        .flat_map(|step| {
            std::iter::once(step.label).chain(step.details.iter().map(|detail| detail.label))
        })
        .collect()
}

#[test]
fn a_console_line_closes_only_when_its_step_has_ended() {
    let waiting = hosting(None, &NetState::Idle);
    assert_eq!(
        ends(&waiting),
        [
            StepEnd::Passed,
            StepEnd::Passed,
            StepEnd::Running,
            StepEnd::Running,
            StepEnd::Running
        ]
    );
    let mut clock = ConsoleClock::new(0.0);
    clock.observe(&waiting, 0.0);
    let long_after = clock.view(&waiting, 10_000.0, true);
    let last = long_after.lines.last().expect("a line");
    assert_eq!(last.label, "SESSION");
    assert_eq!(last.word, None);
    assert!(last.waiting);
    assert_eq!(long_after.cursor_line, Some(long_after.lines.len() - 1));
    assert!(!long_after.lobby_due);
    assert!(!long_after.moving, "a line held by real work types nothing");
    assert!(long_after.counting, "it counts the time it waits instead");
    let waited = last.waited_ms.expect("a running time");
    assert!((waited - (10_000.0 - last.began_ms)).abs() < 0.5);

    let done = hosting(Some(Ok(TICKET)), &hosted());
    clock.observe(&done, 10_000.0);
    let at_once = clock.view(&done, 10_000.0, true);
    assert!(at_once.lines.iter().all(|line| line.typed_share >= 1.0));
    assert_eq!(
        at_once.lines.last().map(|line| line.kind),
        Some(ConsoleLineKind::Closing)
    );
    assert!(at_once.lobby_due, "late lines add no time beyond the work");

    let answered = host_answered_at(400.0);
    let joined = joining(Some(Ok(())), &NetState::Joining, None, &answered);
    assert_eq!(
        ends(&joined),
        [
            StepEnd::Passed,
            StepEnd::Passed,
            StepEnd::Running,
            StepEnd::Running,
            StepEnd::Running
        ],
        "the host answered but has not read the hello yet"
    );
}

#[test]
fn the_console_keeps_a_floor_when_the_work_is_instant() {
    let done = hosting(Some(Ok(TICKET)), &hosted());
    let mut clock = ConsoleClock::new(0.0);
    clock.observe(&done, 0.0);
    let first = clock.view(&done, f64::from(CONSOLE_LINE_TYPING_MS) / 2.0, true);
    assert_eq!(first.lines.len(), 1);
    assert_eq!(first.lines[0].word, None);
    assert!(first.moving);
    let before_floor = clock.view(&done, f64::from(CONSOLE_SHORTEST_RUN_MS) - 1.0, true);
    assert!(
        before_floor
            .lines
            .iter()
            .all(|line| line.typed_share >= 1.0)
    );
    assert!(!before_floor.lobby_due);
    assert!(
        clock
            .view(&done, f64::from(CONSOLE_SHORTEST_RUN_MS), true)
            .lobby_due
    );
}

#[test]
fn a_failed_step_keeps_the_console_open() {
    let failed = hosting(Some(Err("could not open the session")), &NetState::Idle);
    let mut clock = ConsoleClock::new(0.0);
    clock.observe(&failed, 0.0);
    let view = clock.view(&failed, 60_000.0, true);
    assert!(view.failed);
    assert!(!view.lobby_due);
    let last = view.lines.last().expect("a line");
    assert_eq!(last.label, "SESSION");
    assert!(last.failed);
    assert_eq!(view.error.as_deref(), Some("could not open the session"));

    let versions = joining(
        Some(Ok(())),
        &NetState::Failed(judge_rejection_text(RejectReason::VersionMismatch).to_owned()),
        Some(RejectReason::VersionMismatch),
        &host_answered_at(400.0),
    );
    assert_eq!(
        ends(&versions),
        [
            StepEnd::Passed,
            StepEnd::Passed,
            StepEnd::Failed,
            StepEnd::Running,
            StepEnd::Running
        ]
    );
    let seat = joining(
        Some(Ok(())),
        &NetState::Idle,
        Some(RejectReason::SessionFull),
        &host_answered_at(400.0),
    );
    assert_eq!(seat.steps[3].end, StepEnd::Failed);
    let silent = joining(
        Some(Ok(())),
        &NetState::Failed("the host did not answer".to_owned()),
        None,
        &[],
    );
    assert_eq!(silent.steps[1].end, StepEnd::Failed);
    assert_eq!(silent.error.as_deref(), Some("the host did not answer"));
}

#[test]
fn no_motion_shows_the_console_whole() {
    let done = hosting(Some(Ok(TICKET)), &hosted());
    let mut clock = ConsoleClock::new(0.0);
    clock.observe(&done, 0.0);
    let view = clock.view(&done, 0.0, false);
    assert!(view.lines.iter().all(|line| line.typed_share >= 1.0));
    assert!(
        view.lobby_due,
        "with motion off the lobby shows once the work is done"
    );
    assert!(!view.moving);

    let waiting = hosting(None, &NetState::Idle);
    let mut clock = ConsoleClock::new(0.0);
    clock.observe(&waiting, 0.0);
    let view = clock.view(&waiting, 0.0, false);
    let last = view.lines.last().expect("a line");
    assert_eq!(last.label, "SESSION");
    assert!((last.typed_share - 1.0).abs() < f32::EPSILON);
    assert_eq!(last.word, None);
}

#[test]
fn a_skip_shows_the_end_state_at_once() {
    let done = hosting(Some(Ok(TICKET)), &hosted());
    let mut clock = ConsoleClock::new(0.0);
    clock.observe(&done, 0.0);
    clock.skip();
    assert!(clock.view(&done, 1.0, true).lobby_due);
}

#[test]
fn a_fact_the_launcher_does_not_hold_gets_no_line() {
    let new_game = hosting_with(Some(Ok(TICKET)), &hosted(), Some(None), None);
    let shown = labels(&new_game);
    for absent in ["EXECUTABLE", "UPDATE", "SAVE", "LEVEL", "PLAY TIME"] {
        assert!(!shown.contains(&absent), "{absent} has no fact behind it");
    }
    assert!(shown.contains(&"PLAYER"));

    let save = ConsoleSave {
        slot: 2,
        level: 12,
        play_time: "22 h 10",
    };
    let saved = hosting_with(Some(Ok(TICKET)), &hosted(), Some(Some(save)), None);
    let session = &saved.steps[2];
    assert_eq!(session.value.as_deref(), Some("SAVED GAME"));
    let save_line = session
        .details
        .iter()
        .find(|detail| detail.label == "SAVE")
        .expect("the save is told");
    assert_eq!(save_line.value, "SLOT 03");

    let script = joining(Some(Ok(())), &NetState::Joining, None, &[]);
    assert_eq!(script.steps[1].value, None, "no host name was told");
    assert!(
        script.events.is_empty(),
        "a library that tells nothing gets no line"
    );

    let waiting = hosting(None, &NetState::Idle);
    assert!(
        waiting.steps[2].details.is_empty(),
        "a step tells what it found only once it has passed"
    );
}

#[test]
fn the_meter_counts_closed_steps() {
    let waiting = hosting(None, &NetState::Idle);
    let mut clock = ConsoleClock::new(0.0);
    clock.observe(&waiting, 0.0);
    let view = clock.view(&waiting, 10_000.0, true);
    assert_eq!(view.counted_steps, 4);
    assert_eq!(view.closed_steps, 2);

    let done = hosting(Some(Ok(TICKET)), &hosted());
    let mut clock = ConsoleClock::new(0.0);
    clock.observe(&done, 0.0);
    assert_eq!(clock.view(&done, 0.0, true).closed_steps, 0);
    assert_eq!(clock.view(&done, 10_000.0, true).closed_steps, 4);
}

#[test]
fn a_relay_that_answers_late_does_not_hold_the_ticket() {
    let told = [
        at(
            5.0,
            LinkProgress::Bound {
                ipv4_port: Some(51820),
                ipv6_port: Some(51821),
            },
        ),
        at(5.0, LinkProgress::OwnId("4b1d0c9e2f".to_owned())),
        at(
            300.0,
            LinkProgress::RelayChosen {
                relay: "euc1-1".to_owned(),
            },
        ),
        at(
            4_000.0,
            LinkProgress::RelayAnswered {
                relay: "euc1-1".to_owned(),
                after: Duration::from_millis(3_700),
            },
        ),
    ];
    let script_at = |now: f64| {
        let progress: Vec<TimedProgress> = told
            .iter()
            .filter(|timed| timed.at_ms <= now)
            .cloned()
            .collect();
        let (answer, state) = if now >= 320.0 {
            (Some(Ok(TICKET)), hosted())
        } else {
            (None, NetState::Idle)
        };
        hosting_told(answer, &state, Some(None), Some("game.exe"), &progress)
    };
    let frames: Vec<f64> = (0..=60).map(|frame| f64::from(frame) * 25.0).collect();
    let (_, before_the_answer) = view_as_it_came(script_at, &frames);
    assert!(
        before_the_answer.lobby_due,
        "the lobby shows on the floor, not on the relay's answer"
    );
    let shown = shown_labels(&before_the_answer);
    let session = shown
        .iter()
        .position(|label| *label == "SESSION")
        .expect("SESSION");
    let ticket = shown
        .iter()
        .position(|label| *label == "TICKET")
        .expect("TICKET");
    for label in ["BOUND", "LINK ID", "RELAY CHOSEN"] {
        let place = shown.iter().position(|shown| *shown == label).expect(label);
        assert!(
            session < place && place < ticket,
            "{label} sits under SESSION"
        );
    }
    assert!(!shown.contains(&"RELAY ANSWERED"));
    let bound = &before_the_answer.lines[session + 1];
    assert!(
        bound.began_ms < 10.0,
        "a line keeps the time its event happened"
    );

    let frames: Vec<f64> = (0..=180).map(|frame| f64::from(frame) * 25.0).collect();
    let (_, after) = view_as_it_came(script_at, &frames);
    let shown = shown_labels(&after);
    let begins = shown
        .iter()
        .position(|label| *label == "BEGINS")
        .expect("BEGINS");
    let answered = shown
        .iter()
        .position(|label| *label == "RELAY ANSWERED")
        .expect("the late answer is told");
    assert!(
        answered > begins,
        "a late event comes after the lines already shown"
    );
    let line = &after.lines[answered];
    assert_eq!(line.value.as_deref(), Some("EUC1-1 IN 3700 MS"));
}

#[test]
fn a_direct_path_that_never_comes_still_seats_the_guest() {
    let progress = welcomed(host_answered_at(400.0), 450.0);
    let script = joining(Some(Ok(())), &connected(), None, &progress);
    assert!(script.finished());
    assert_eq!(script.steps[1].value.as_deref(), Some(HOST));
    let won: Vec<&str> = script
        .events
        .iter()
        .filter(|event| event.label == "PATH WON")
        .map(|event| event.value.as_str())
        .collect();
    assert_eq!(won, ["RELAYED  62 MS"]);
    assert!(
        script
            .events
            .iter()
            .filter(|event| event.label.starts_with("PATH"))
            .all(|event| !event.value.contains("DIRECT")),
        "no direct path is told when none came"
    );
    let mut clock = ConsoleClock::new(0.0);
    clock.observe(&script, 500.0);
    let view = clock.view(&script, 5_000.0, true);
    assert!(view.lobby_due);
    let shown = shown_labels(&view);
    let host = shown
        .iter()
        .position(|label| *label == "HOST")
        .expect("HOST");
    let versions = shown
        .iter()
        .position(|label| *label == "VERSIONS")
        .expect("VERSIONS");
    let tried = shown
        .iter()
        .position(|label| *label == "PATH TRIED")
        .expect("PATH TRIED");
    assert!(host < tried && tried < versions);
    for label in [
        "HOST ID",
        "HOST RELAY",
        "HOST ADDRESSES",
        "HELLO SENT",
        "WELCOME READ",
        "HOST NAME",
    ] {
        assert!(shown.contains(&label), "{label} is told");
    }
}

#[test]
fn a_refused_version_fails_after_the_hello_it_answers() {
    let refusal = judge_rejection_text(RejectReason::VersionMismatch);
    let state_at = |now: f64| {
        if now >= 900.0 {
            NetState::Failed(refusal.to_owned())
        } else {
            NetState::Joining
        }
    };
    let script_at = |now: f64| {
        let progress: Vec<TimedProgress> = host_answered_at(400.0)
            .into_iter()
            .filter(|timed| timed.at_ms <= now)
            .collect();
        let rejected = (now >= 900.0).then_some(RejectReason::VersionMismatch);
        joining(Some(Ok(())), &state_at(now), rejected, &progress)
    };
    let frames: Vec<f64> = (0..=80).map(|frame| f64::from(frame) * 25.0).collect();
    let (script, view) = view_as_it_came(script_at, &frames);
    assert_eq!(script.steps[2].end, StepEnd::Failed);
    assert!(view.failed);
    assert_eq!(view.error.as_deref(), Some(refusal));
    let shown = shown_labels(&view);
    assert_eq!(shown.last(), Some(&"HELLO SENT"));
    let versions = shown
        .iter()
        .position(|label| *label == "VERSIONS")
        .expect("VERSIONS");
    assert!(view.lines[versions].failed);
    let hello = shown
        .iter()
        .position(|label| *label == "HELLO SENT")
        .expect("HELLO SENT");
    assert!(
        hello > versions,
        "the hello sits under the step it belongs to"
    );
}
