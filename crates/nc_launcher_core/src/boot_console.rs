mod console_clock;
mod console_script;

pub use console_clock::{ConsoleClock, ConsoleLineKind, ConsoleLineView, ConsoleView};
pub use console_script::{
    ConsoleDetail, ConsoleEvent, ConsoleGameFacts, ConsoleSave, ConsoleScript, ConsoleStep,
    ConsoleTask, HostingFacts, JoiningFacts, StepEnd, TimedProgress, console_events,
    hosting_script, joining_script,
};

pub const CONSOLE_LINE_TYPING_MS: f32 = 70.0;
pub const CONSOLE_SHORTEST_RUN_MS: f32 = 1400.0;
