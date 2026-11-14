use std::time::Instant;

use nc_launcher_core::{ConsoleClock, ConsoleTask};

/// The boot console on screen: which wait it covers, its clock, and the moment it opened on the clock the
/// network's progress is stamped with.
pub struct ConsoleRun {
    pub task: ConsoleTask,
    pub clock: ConsoleClock,
    pub started_at: Instant,
}
