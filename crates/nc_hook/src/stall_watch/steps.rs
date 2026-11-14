use std::sync::Mutex;

static STEPS: Mutex<GameThreadSteps> = Mutex::new(GameThreadSteps {
    current: None,
    last_finished: None,
});

/// The mod step the game thread is inside, and the last one it finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GameThreadSteps {
    pub current: Option<&'static str>,
    pub last_finished: Option<&'static str>,
}

struct StepGuard {
    name: &'static str,
    outer: Option<&'static str>,
}

impl Drop for StepGuard {
    fn drop(&mut self) {
        let mut steps = STEPS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        steps.current = self.outer;
        steps.last_finished = Some(self.name);
    }
}

/// Runs one named step of the mod's work on the game thread, so a stall sample can say which
/// step the thread stopped in.
pub fn in_game_thread_step<R>(name: &'static str, run: impl FnOnce() -> R) -> R {
    let outer = {
        let mut steps = STEPS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        steps.current.replace(name)
    };
    let _guard = StepGuard { name, outer };
    run()
}

/// The steps as last written; `None` when the game thread holds the record right now (a
/// watcher must never wait on a thread that may be stopped).
pub fn game_thread_steps() -> Option<GameThreadSteps> {
    STEPS.try_lock().ok().map(|steps| *steps)
}
