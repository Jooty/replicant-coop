mod callback_gate;
mod hooks;
mod message_handlers;
mod restart_signals;
mod scene_follow;
mod shared_functions;
mod start_hold;
mod state;
mod tick;

pub(crate) use callback_gate::{
    control_slots_free_now, drop_start_of_a_played_scene, forget_gate_copies,
    note_relayed_scene_played, park_partner_call, partner_calls_waiting, partner_end_waiting_for,
};
pub use callback_gate::{
    end_ran_here, hold_fired_callback, latest_partner_end, note_talk_command,
    take_player_talk_command,
};
pub use hooks::install;
pub(crate) use message_handlers::{expect_handler_run, forget_handler_run, run_message_handler};
pub(crate) use restart_signals::restart_signals;
pub use scene_follow::{is_injected_duplicate, note_fired_callback};
pub(crate) use shared_functions::{
    close_menus_for_modal_entry, on_function_traced, run_shared_function,
};
pub(crate) use state::event_scene_running;
pub use state::local_in_cutscene;
pub(crate) use state::partner_in_cutscene;
pub use tick::tick;
