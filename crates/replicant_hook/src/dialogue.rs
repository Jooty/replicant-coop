mod choice_hover;
mod choices;
mod hooks;
mod tags;
mod talker_name;
mod tick;
mod watcher_hold;
mod window_calls;

pub(crate) use choice_hover::shown_choice_tags;
pub use hooks::install;
pub(crate) use tick::own_talk_window_open;
pub use tick::{local_activity, tick_dialogue};
