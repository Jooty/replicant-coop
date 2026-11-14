mod guest_follow;
mod host_watch;
mod motion;
mod scan;
mod state;
mod steer;
mod text;

pub use guest_follow::{CartFollowStep, GuestCartFollow, OwnCartFacts};
pub use host_watch::{CartRideEvent, CartRideFacts, HostRideWatch, RiddenCart};
pub use motion::{CartMotion, read_cart_motion, read_cart_run_end_frame};
pub use replicant_proto::CartRideEnd;
pub use scan::{
    CartSighting, cart_is_live, match_cart, match_ride_cart, read_cart_state, scan_cart_actors,
};
pub use state::{CartState, cart_state, parked_state_after, run_state_from};
pub use steer::{
    CartLeftAlone, CartSteer, CartSteerSample, HostCartRun, HostCartRunTrack, steer_cart,
};
pub use text::{
    cart_follow_step_text, cart_frame_set_text, cart_ride_event_text, cart_run_end_sent_text,
    carts_here_text, guest_ride_refused_text, host_cart_run_text, host_run_ended_text,
    partner_wants_to_ride_text, riding_along_text,
};

pub const CART_VTABLE_RVA: u32 = 0x00B0_7068;
pub const CART_STATE_OFFSET: u64 = 0x1_4D84;
pub const CART_SET_STATE_FUNCTION_RVA: u32 = 0x0016_1070;
pub const CART_MESSAGE_FUNCTION_RVA: u32 = 0x0016_8430;
pub const CART_BOARD_MESSAGE_ID: u32 = 0x11;
pub const PLAYER_ON_CART_FLAG_OFFSET: u64 = 0x6_B74C;
pub const FIRST_PARKED_CART_STATE: u8 = 1;
pub const LAST_PARKED_CART_STATE: u8 = 4;
pub const FIRST_RUNNING_CART_STATE: u8 = 5;
pub const LAST_RUNNING_CART_STATE: u8 = 8;
pub const CART_SCAN_INTERVAL_MS: u64 = 100;
pub const CART_MATCH_UNITS: f32 = 8.0;
pub const CART_RIDER_LIFT_UNITS: f32 = 0.3;
pub const CART_BOARD_SETTLE_MS: u64 = 200;
pub const CART_BOARD_CONFIRM_MS: u64 = 500;
pub const CART_BOARD_TRIES: u8 = 3;
pub const CART_OWN_RIDE_LONGEST_MS: u64 = 60_000;
pub const CART_BOARD_GIVE_UP_MS: u64 = 6_000;
pub const CART_END_GRACE_MS: u64 = 3_000;
pub const CART_END_PULL_DELAY_MS: u64 = 2_500;
pub const CART_PARTNER_NEAR_UNITS: f32 = 10.0;
pub const CART_RIDING_LOG_INTERVAL_MS: u64 = 1_000;
pub const CART_RUN_END_FRAME_OFFSET: u64 = 0x1_4D80;
pub const CART_UPDATE_FUNCTION_RVA: u32 = 0x0016_4590;
pub const CART_SET_MOTION_RATE_FUNCTION_RVA: u32 = 0x000F_AE90;
pub const OLD_MOTION_LAYER_SET_FRAME_FUNCTION_RVA: u32 = 0x0040_2D20;
pub const NEWER_MOTION_LAYER_SET_FRAME_FUNCTION_RVA: u32 = 0x0084_21A0;
pub const CART_RUN_MOTION_LAYER: i32 = 0;
pub const CART_NEWER_MOTION_FLAG_OFFSET: u64 = 0x8B20;
pub const CART_NEWER_MOTION_LAYER_COUNT_OFFSET: u64 = 0x1674;
pub const CART_NEWER_MOTION_LAYERS_OFFSET: u64 = 0x1678;
pub const CART_OLD_MOTION_LAYER_COUNT_OFFSET: u64 = 0x8BAC;
pub const CART_OLD_MOTION_LAYERS_OFFSET: u64 = 0x8BB0;
pub const MOTION_LAYER_SLOT_INDEX_OFFSET: u64 = 0x8;
pub const MOTION_LAYER_SLOTS_OFFSET: u64 = 0x10;
pub const OLD_MOTION_SLOT_BYTES: u64 = 0x48;
pub const NEWER_MOTION_SLOT_BYTES: u64 = 600;
pub const MOTION_SLOT_FRAME_OFFSET: u64 = 0x4;
pub const OLD_MOTION_SLOT_RATE_OFFSET: u64 = 0x14;
pub const NEWER_MOTION_SLOT_RATE_OFFSET: u64 = 0x10;
pub const CART_RUN_END_FRAME_WHEN_UNREAD: f32 = 7_185.0;
pub const CART_RUN_FRAMES_PER_SECOND_AT_RATE_ONE: f32 = 60.0;
pub const CART_STEER_RATE_PER_FRAME_OF_GAP: f32 = 1.0 / 30.0;
pub const CART_STEERED_RATE_MIN: f32 = 0.1;
pub const CART_STEERED_RATE_MAX: f32 = 2.5;
pub const CART_FRAME_SET_GAP_FRAMES: f32 = 30.0;
pub const CART_FRAME_SET_STOPS_FRAMES_BEFORE_END: f32 = 5.0;
pub const CART_HOST_RUN_SILENT_MS: u64 = 1_000;
pub const CART_HOST_RUN_STILL_MS: u64 = 400;
pub const CART_HOST_RUN_STILL_FRAMES: f32 = 0.01;
pub const CART_RUN_LONGEST_EXTRAPOLATION_MS: u64 = 300;
pub const CART_REFUSED_NOTICE_INTERVAL_MS: u64 = 3_000;
pub const GUEST_CART_REFUSED_TEXT: &str = "Only the host starts a cart ride";
