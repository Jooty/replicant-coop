use crate::crash_report::{ModuleSpot, StackFrame};
use crate::stall_watch::GameThreadSteps;

/// Where a game thread that stopped ticking stood when the watcher froze it for a moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StallSample {
    pub thread_id: u32,
    pub stalled_ms: u64,
    pub index: u32,
    pub instruction: u64,
    pub instruction_module: Option<ModuleSpot>,
    pub stack: Vec<StackFrame>,
    pub steps: Option<GameThreadSteps>,
}

pub fn stall_sample_text(sample: &StallSample) -> String {
    let mut text = format!(
        "stall: the game thread {} has not ticked for {} ms (sample {}); no exception was raised, it is a hang or a long load\n",
        sample.thread_id, sample.stalled_ms, sample.index
    );
    match &sample.instruction_module {
        Some(module) => text.push_str(&format!(
            "stall: it stands at 0x{:016X} in {} rva 0x{:X}\n",
            sample.instruction,
            module.name,
            sample.instruction.wrapping_sub(module.base)
        )),
        None => text.push_str(&format!(
            "stall: it stands at 0x{:016X}, in no loaded module\n",
            sample.instruction
        )),
    }
    let steps = match sample.steps {
        None => "unreadable (the thread holds the step record)".to_owned(),
        Some(GameThreadSteps {
            current: Some(current),
            ..
        }) => format!("inside the mod's step \"{current}\""),
        Some(GameThreadSteps {
            current: None,
            last_finished: Some(last),
        }) => format!("in game code; the mod's last finished step was \"{last}\""),
        Some(GameThreadSteps {
            current: None,
            last_finished: None,
        }) => "in game code; no mod step has run yet".to_owned(),
    };
    text.push_str(&format!("stall: mod step: {steps}\n"));
    for (index, frame) in sample.stack.iter().enumerate() {
        text.push_str(&format!(
            "stall: stack {index} 0x{:016X} {} rva 0x{:X}\n",
            frame.address,
            frame.module.name,
            frame.address.wrapping_sub(frame.module.base)
        ));
    }
    text
}

pub fn stall_recovered_line(stalled_ms: u64, samples: u32) -> String {
    format!(
        "stall: the game thread ticks again after {stalled_ms} ms ({samples} samples taken): a long stall, not a hang"
    )
}

pub fn game_thread_ended_line(thread_id: u32, exit_code: u32) -> String {
    format!(
        "stall: the game thread {thread_id} has ended (exit code 0x{exit_code:X}) while the process lives on"
    )
}

pub fn stall_unreadable_line(thread_id: u32, stalled_ms: u64, why: &str) -> String {
    format!(
        "stall: the game thread {thread_id} has not ticked for {stalled_ms} ms and could not be sampled: {why}"
    )
}
