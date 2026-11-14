mod body_choice;
mod follow;
mod guest_wait;
mod host_options;
mod name;
mod plan;
mod players;
mod slots;
mod start;

pub use body_choice::{body_label, character_id_for_body};
pub use follow::{
    FollowNotice, GAME_QUIT_GRACE_SECONDS, HostClosedInputs, LastLaunch, QuitFallback, QuitStep,
    host_game_closed, own_close_notice, relaunch_choice,
};
pub use guest_wait::{GUEST_WAIT_FOR_HOST_GAME_SECONDS, GuestWaitStep, GuestWaiting};
pub use host_options::host_options_editable;
pub use name::{MAX_NAME_BYTES, NameError, validate_name};
pub use nc_launcher_core::{PlayerStatus, ReadyInputs, lobby_ready, player_status};
pub use plan::{LaunchMode, resolve_mode};
pub use players::{PLAYER_TABLE_HEADERS, PlayerRole, PlayerRow, RowInputs, player_rows};
pub use slots::new_game_slot;
pub use start::{guest_accepts_start, start_enabled};
