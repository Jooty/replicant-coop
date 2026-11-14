mod arrival;
mod fire;
mod passes;
mod registration;
mod registry;

pub use arrival::{HandlerArrival, HandlerArrivalFacts, HandlerDrop, handler_arrival};
pub use fire::{HandlerFire, HandlerFireFacts, handler_fire};
pub use passes::RelayedHandlerPasses;
pub use registration::{
    handler_name_from_parts, message_id_from_parts, message_id_tag_range, message_id_value_range,
    message_name_tag_range, message_name_text_range,
};
pub use registry::{HandlerRegistered, MessageHandlers, is_player_local_handler};

pub const SET_MESSAGE_CALLBACK_NATIVE_RVA: u32 = 0x6F_DFD0;
pub const SET_MESSAGE_CALLBACK_NAME_ARG_INDEX: usize = 0;
pub const SET_MESSAGE_CALLBACK_MESSAGE_ID_ARG_INDEX: usize = 3;
pub const MAX_MESSAGE_HANDLERS: usize = 1024;
pub const MAX_RELAYED_HANDLER_PASSES: usize = 64;
pub const RELAYED_HANDLER_PASS_MS: u64 = 20_000;
pub const PLAYER_LOCAL_MESSAGE_IDS: [i32; 7] = [210, 215, 239, 470, 471, 472, 473];
pub const PLAYER_LOCAL_HANDLER_NAMES: [&str; 2] = ["LIB_GameOver", "SQ_Fishing_Radio"];
