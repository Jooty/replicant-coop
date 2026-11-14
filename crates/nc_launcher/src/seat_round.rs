mod guest_round;
mod host_round;
mod shown_id;

pub const HOST_MEMBER_SLOT: u8 = 0;
pub const CAMPAIGN_ID_SHOWN_CHARACTERS: usize = 8;

pub use guest_round::GuestSeatRound;
pub use host_round::{HostSeatRound, HostStartStep, PlayingCampaign};
pub use shown_id::shown_campaign_id;
