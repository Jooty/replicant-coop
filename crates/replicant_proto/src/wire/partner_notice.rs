use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartnerNotice {
    WantsToChangeZone,
    HostWantsToGo { phase: Option<u16> },
    RejoiningYourArea,
    WantsToRideTheCart,
}
