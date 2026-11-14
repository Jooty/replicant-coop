use nc_net::{LinkProgress, LookupMeans, PathRoute};
use replicant_proto::RejectReason;

use crate::event::NetState;
use crate::report::judge_rejection_text;

const FAILED_WORD: &str = "FAILED";
const LOBBY_LABEL: &str = "LOBBY";
const LOBBY_WORD: &str = "READY";
const TICKET_START_CHARACTERS: usize = 12;
const ELLIPSIS: char = '\u{2026}';
const HOSTING_NETWORK_STEP: usize = 2;
const JOINING_NETWORK_STEP: usize = 1;

/// Which wait the boot console covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleTask {
    Hosting,
    Joining,
}

/// Where one step of the wait stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepEnd {
    Running,
    Passed,
    Failed,
}

/// A fact under a step: what the step found or set up, told once the step has passed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleDetail {
    pub label: &'static str,
    pub value: String,
}

/// One console step: a label, the value it stands for when the launcher holds one, the status word it closes
/// with once it has ended, and the facts under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleStep {
    pub label: &'static str,
    pub value: Option<String>,
    pub word: &'static str,
    pub end: StepEnd,
    pub details: Vec<ConsoleDetail>,
}

/// One thing the network did, as the console tells it, and when it happened on the caller's clock.
#[derive(Debug, Clone, PartialEq)]
pub struct ConsoleEvent {
    pub label: &'static str,
    pub value: String,
    pub at_ms: f64,
}

/// A progress entry of the networking library, placed on the caller's clock.
#[derive(Debug, Clone, PartialEq)]
pub struct TimedProgress {
    pub at_ms: f64,
    pub progress: LinkProgress,
}

/// The steps of one wait in order, the last one closing the console; the network's events in the order they
/// happened, told under the step from `events_from_step` on that is running when each comes; and the error
/// text of a failed step.
#[derive(Debug, Clone, PartialEq)]
pub struct ConsoleScript {
    pub steps: Vec<ConsoleStep>,
    pub events: Vec<ConsoleEvent>,
    pub events_from_step: usize,
    pub error: Option<String>,
}

/// The save a hosted saved game continues: its slot counted from 0, its level and its play time as text.
#[derive(Debug, Clone, Copy)]
pub struct ConsoleSave<'a> {
    pub slot: u8,
    pub level: i32,
    pub play_time: &'a str,
}

/// The launcher's own facts about the game and the mod, held before any network work.
#[derive(Debug, Clone, Copy)]
pub struct ConsoleGameFacts<'a> {
    pub protocol: u16,
    pub installed_build: Option<&'a str>,
    pub waiting_update: Option<&'a str>,
    pub player_name: &'a str,
}

/// What hosting has done so far: the two checks made before the session is asked for (the folder's path or
/// the error text), the answer of the host call (the ticket or the error text) once it has come, the
/// session's state, what the network has done, and the facts the session is opened with. `saved_game` is
/// `None` when the kind of game is not known, `Some(None)` for a new game.
#[derive(Debug, Clone, Copy)]
pub struct HostingFacts<'a> {
    pub game_folder: Result<&'a str, &'a str>,
    pub executable: Option<&'a str>,
    pub mod_files: Result<(), &'a str>,
    pub hook_file: &'a str,
    pub game: ConsoleGameFacts<'a>,
    pub saved_game: Option<Option<ConsoleSave<'a>>>,
    pub seats: u8,
    pub enemy_health_percent: u16,
    pub enemy_damage_percent: u16,
    pub host_answer: Option<Result<&'a str, &'a str>>,
    pub state: &'a NetState,
    pub progress: &'a [TimedProgress],
}

/// What joining has done so far: the ticket and its own check, the join call's answer, the session's state
/// and the host's refusal if it refused, what the network has done, and the seat the session gave this
/// player.
#[derive(Debug, Clone, Copy)]
pub struct JoiningFacts<'a> {
    pub ticket_text: &'a str,
    pub ticket: Result<(), &'a str>,
    pub game: ConsoleGameFacts<'a>,
    pub join_answer: Option<Result<(), &'a str>>,
    pub state: &'a NetState,
    pub rejected: Option<RejectReason>,
    pub progress: &'a [TimedProgress],
    pub own_slot: Option<u8>,
}

enum Outcome<'a> {
    Running,
    Passed,
    Failed(&'a str),
}

struct Planned<'a> {
    label: &'static str,
    value: Option<String>,
    word: &'static str,
    outcome: Outcome<'a>,
    details: Vec<ConsoleDetail>,
}

impl ConsoleScript {
    pub fn failed(&self) -> bool {
        self.steps.iter().any(|step| step.end == StepEnd::Failed)
    }

    pub fn finished(&self) -> bool {
        self.steps.iter().all(|step| step.end == StepEnd::Passed)
    }

    /// The steps that count toward the meter: every step but the closing one.
    pub fn counted_steps(&self) -> usize {
        self.steps.len().saturating_sub(1)
    }
}

/// The hosting wait: the game is found, the mod's files are in place, the session is open, the ticket is
/// made.
pub fn hosting_script(facts: HostingFacts) -> ConsoleScript {
    let session_open = match (facts.host_answer, facts.state) {
        (Some(Err(text)), _) => Outcome::Failed(text),
        (Some(Ok(_)), _) | (None, NetState::Hosting { .. } | NetState::Connected { .. }) => {
            Outcome::Passed
        }
        _ => Outcome::Running,
    };
    let ticket = match (facts.host_answer, facts.state) {
        (Some(Ok(ticket)), _) if !ticket.is_empty() => Some(ticket),
        (_, NetState::Hosting { ticket }) if !ticket.is_empty() => Some(ticket.as_str()),
        _ => None,
    };
    let folder = Planned {
        label: "GAME FOLDER",
        value: facts.game_folder.ok().map(str::to_owned),
        word: "FOUND",
        outcome: checked(facts.game_folder.map(|_| ())),
        details: details([("EXECUTABLE", facts.executable.map(str::to_owned))]),
    };
    let mod_files = Planned {
        label: "MOD FILES",
        value: facts.game.installed_build.map(build_value),
        word: "PRESENT",
        outcome: checked(facts.mod_files),
        details: details([
            ("FILE", Some(facts.hook_file.to_owned())),
            ("UPDATE", waiting_update(facts.game)),
            ("PROTOCOL", Some(facts.game.protocol.to_string())),
        ]),
    };
    let save = facts.saved_game.flatten();
    let session = Planned {
        label: "SESSION",
        value: facts.saved_game.map(|saved| {
            if saved.is_some() {
                "SAVED GAME".to_owned()
            } else {
                "NEW GAME".to_owned()
            }
        }),
        word: "OPEN",
        outcome: session_open,
        details: details([
            ("PLAYER", Some(facts.game.player_name.to_owned())),
            (
                "SAVE",
                save.map(|save| format!("SLOT {:02}", u16::from(save.slot) + 1)),
            ),
            ("LEVEL", save.map(|save| save.level.to_string())),
            ("PLAY TIME", save.map(|save| save.play_time.to_owned())),
            ("SEATS", Some(facts.seats.to_string())),
            (
                "ENEMY HEALTH",
                Some(per_extra_player(facts.enemy_health_percent)),
            ),
            (
                "ENEMY DAMAGE",
                Some(per_extra_player(facts.enemy_damage_percent)),
            ),
        ]),
    };
    let ticket_line = Planned {
        label: "TICKET",
        value: ticket.map(ticket_length),
        word: "ISSUED",
        outcome: if ticket.is_some() {
            Outcome::Passed
        } else {
            Outcome::Running
        },
        details: details([("BEGINS", ticket.map(ticket_start))]),
    };
    in_order(
        vec![folder, mod_files, session, ticket_line],
        console_events(facts.progress),
        HOSTING_NETWORK_STEP,
    )
}

/// The joining wait: the ticket is read, the host answers, it takes this launcher's hello (the versions
/// match), the seat is taken.
pub fn joining_script(facts: JoiningFacts) -> ConsoleScript {
    let failure: Option<&str> = match (facts.join_answer, facts.rejected, facts.state) {
        (Some(Err(text)), _, _) => Some(text),
        (_, Some(reason), _) => Some(judge_rejection_text(reason)),
        (_, _, NetState::Failed(text)) => Some(text),
        _ => None,
    };
    let connected = matches!(facts.state, NetState::Connected { .. });
    let welcome_host = facts
        .progress
        .iter()
        .find_map(|timed| match &timed.progress {
            LinkProgress::WelcomeRead { host_name, .. } => Some(host_name.clone()),
            _ => None,
        });
    let welcomed = connected || welcome_host.is_some();
    let answered = facts
        .progress
        .iter()
        .any(|timed| matches!(timed.progress, LinkProgress::HostAnswered { .. }));
    let versions_refused = matches!(
        facts.rejected,
        Some(RejectReason::BuildMismatch | RejectReason::VersionMismatch | RejectReason::Malformed)
    );
    let otherwise = |done: bool| match (done, failure) {
        (true, _) => Outcome::Passed,
        (false, Some(text)) => Outcome::Failed(text),
        (false, None) => Outcome::Running,
    };
    let host_reached = otherwise(answered || welcomed || facts.rejected.is_some());
    let versions_match = if versions_refused {
        Outcome::Failed(failure.unwrap_or_default())
    } else {
        otherwise(welcomed || facts.rejected.is_some())
    };
    let seat_taken = otherwise(connected && facts.rejected.is_none() && facts.own_slot.is_some());
    let ticket_text = facts.ticket_text.trim();
    let ticket_held = (!ticket_text.is_empty()).then_some(ticket_text);
    let ticket = Planned {
        label: "TICKET",
        value: ticket_held.map(ticket_length),
        word: "READ",
        outcome: checked(facts.ticket),
        details: details([("BEGINS", ticket_held.map(ticket_start))]),
    };
    let peer_name = match facts.state {
        NetState::Connected { peer_name } if !peer_name.is_empty() => Some(peer_name.clone()),
        _ => None,
    };
    let host = Planned {
        label: "HOST",
        value: welcome_host.or(peer_name),
        word: "REACHED",
        outcome: host_reached,
        details: Vec::new(),
    };
    let versions = Planned {
        label: "VERSIONS",
        value: Some(format!("PROTOCOL {}", facts.game.protocol)),
        word: "MATCH",
        outcome: versions_match,
        details: details([
            ("MOD", facts.game.installed_build.map(build_value)),
            ("UPDATE", waiting_update(facts.game)),
        ]),
    };
    let seat = Planned {
        label: "SEAT",
        value: facts
            .own_slot
            .filter(|_| connected)
            .map(|slot| format!("{:02}", u16::from(slot) + 1)),
        word: "TAKEN",
        outcome: seat_taken,
        details: details([("PLAYER", Some(facts.game.player_name.to_owned()))]),
    };
    in_order(
        vec![ticket, host, versions, seat],
        console_events(facts.progress),
        JOINING_NETWORK_STEP,
    )
}

/// The network's events as console lines: one line for each thing the library or the session told, and
/// none for what it did not.
pub fn console_events(progress: &[TimedProgress]) -> Vec<ConsoleEvent> {
    let mut events = Vec::new();
    for timed in progress {
        let mut line = |label: &'static str, value: String| {
            events.push(ConsoleEvent {
                label,
                value,
                at_ms: timed.at_ms,
            });
        };
        match &timed.progress {
            LinkProgress::Bound {
                ipv4_port,
                ipv6_port,
            } => {
                let ports: Vec<String> = [("IPV4", ipv4_port), ("IPV6", ipv6_port)]
                    .into_iter()
                    .filter_map(|(family, port)| port.map(|port| format!("{family} PORT {port}")))
                    .collect();
                if !ports.is_empty() {
                    line("BOUND", ports.join(", "));
                }
            }
            LinkProgress::OwnId(id) => line("LINK ID", id.to_uppercase()),
            LinkProgress::RelayChosen { relay } => line("RELAY CHOSEN", relay.to_uppercase()),
            LinkProgress::RelayAnswered { relay, after } => line(
                "RELAY ANSWERED",
                format!("{} IN {} MS", relay.to_uppercase(), after.as_millis()),
            ),
            LinkProgress::DirectAddressFound { kind, count } => line(
                "DIRECT ADDRESS",
                format!("{count}  {}", kind.words().to_uppercase()),
            ),
            LinkProgress::TicketRead {
                host_id,
                relay,
                direct_addresses,
            } => {
                line("HOST ID", host_id.to_uppercase());
                if let Some(relay) = relay {
                    line("HOST RELAY", relay.to_uppercase());
                }
                if *direct_addresses > 0 {
                    line("HOST ADDRESSES", format!("{direct_addresses} DIRECT"));
                }
            }
            LinkProgress::LocalLookupAnswering { port } => {
                line("LOCAL LOOKUP", format!("ANSWERING ON UDP PORT {port}"));
            }
            LinkProgress::LocalLookupOff { port } => {
                line("LOCAL LOOKUP", format!("OFF, UDP PORT {port} IN USE"));
            }
            LinkProgress::KeyPublished { after } => {
                line("KEY PUBLISHED", format!("IN {} MS", after.as_millis()));
            }
            LinkProgress::KeyNotPublished { .. } => {
                line("KEY PUBLISHED", "NOT SEEN, LOCAL NETWORK ONLY".to_owned());
            }
            LinkProgress::LookingUpHost => line("LOOKUP", "BY HOST KEY".to_owned()),
            LinkProgress::HostFound {
                by,
                after,
                relay,
                direct_addresses,
            } => {
                line(
                    "HOST FOUND",
                    format!("{}  {} MS", lookup_words(*by), after.as_millis()),
                );
                if let Some(relay) = relay {
                    line("HOST RELAY", relay.to_uppercase());
                }
                if *direct_addresses > 0 {
                    line("HOST ADDRESSES", format!("{direct_addresses} DIRECT"));
                }
            }
            LinkProgress::HostNotFound { reason } => line("HOST NOT FOUND", reason.to_uppercase()),
            LinkProgress::HostAnswered { after } => {
                line("HOST ANSWERED", format!("IN {} MS", after.as_millis()));
            }
            LinkProgress::DialFailed => {}
            LinkProgress::PathTried(route) => line("PATH TRIED", route_words(*route)),
            LinkProgress::PathWon { route, rtt } => line(
                "PATH WON",
                match rtt {
                    Some(rtt) => format!("{}  {} MS", route_words(*route), rtt.as_millis()),
                    None => route_words(*route),
                },
            ),
            LinkProgress::HelloSent { protocol } => {
                line("HELLO SENT", format!("PROTOCOL {protocol}"));
            }
            LinkProgress::WelcomeRead { seat, host_name } => {
                line("WELCOME READ", format!("SEAT {:02}", u16::from(*seat) + 1));
                line("HOST NAME", host_name.clone());
            }
        }
    }
    events
}

fn lookup_words(by: LookupMeans) -> &'static str {
    match by {
        LookupMeans::LocalNetwork => "LOCAL NETWORK",
        LookupMeans::PublishedRecord => "PUBLISHED RECORD",
        LookupMeans::TicketAddresses => "TICKET ADDRESSES",
    }
}

fn route_words(route: PathRoute) -> String {
    route.words().replace(',', "").to_uppercase()
}

fn checked(check: Result<(), &str>) -> Outcome<'_> {
    match check {
        Ok(()) => Outcome::Passed,
        Err(text) => Outcome::Failed(text),
    }
}

fn details<const N: usize>(facts: [(&'static str, Option<String>); N]) -> Vec<ConsoleDetail> {
    facts
        .into_iter()
        .filter_map(|(label, value)| {
            value
                .filter(|value| !value.trim().is_empty())
                .map(|value| ConsoleDetail { label, value })
        })
        .collect()
}

fn waiting_update(game: ConsoleGameFacts) -> Option<String> {
    game.waiting_update
        .map(|build| format!("{} WAITING", build_value(build)))
}

fn build_value(build: &str) -> String {
    format!("BUILD {build}")
}

fn per_extra_player(percent: u16) -> String {
    format!("+{percent} % PER EXTRA PLAYER")
}

fn ticket_length(ticket: &str) -> String {
    format!("{} CHARACTERS", ticket.chars().count())
}

fn ticket_start(ticket: &str) -> String {
    let mut start: String = ticket.chars().take(TICKET_START_CHARACTERS).collect();
    if ticket.chars().count() > TICKET_START_CHARACTERS {
        start.push(ELLIPSIS);
    }
    start
}

fn in_order(
    planned: Vec<Planned>,
    events: Vec<ConsoleEvent>,
    events_from_step: usize,
) -> ConsoleScript {
    let mut steps = Vec::with_capacity(planned.len() + 1);
    let mut error = None;
    // A step counts only once every step before it has passed: the lines are one sequence.
    let mut open = true;
    for line in planned {
        let end = match line.outcome {
            _ if !open => StepEnd::Running,
            Outcome::Passed => StepEnd::Passed,
            Outcome::Running => StepEnd::Running,
            Outcome::Failed(text) => {
                error = Some(text.to_owned());
                StepEnd::Failed
            }
        };
        open = end == StepEnd::Passed;
        let word = if end == StepEnd::Failed {
            FAILED_WORD
        } else {
            line.word
        };
        let details = if end == StepEnd::Passed {
            line.details
        } else {
            Vec::new()
        };
        steps.push(ConsoleStep {
            label: line.label,
            value: line.value,
            word,
            end,
            details,
        });
    }
    steps.push(ConsoleStep {
        label: LOBBY_LABEL,
        value: None,
        word: LOBBY_WORD,
        end: if open {
            StepEnd::Passed
        } else {
            StepEnd::Running
        },
        details: Vec::new(),
    });
    ConsoleScript {
        steps,
        events,
        events_from_step,
        error,
    }
}
