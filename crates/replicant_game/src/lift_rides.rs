mod guest_call;
mod host_ride;
mod pick;

pub use guest_call::{GuestCallStep, GuestLiftCall, GuestRideEvent, GuestRideFacts};
pub use host_ride::{HostBusy, HostLiftRide, HostRideFacts, HostRideStep, RideEnd};
pub use pick::{LiftRide, lift_ride, warp_pick_origin};

pub const LIFT_PHASES: [&str; 8] = [
    "A_MOUNT_ROBOT_01",
    "A_MOUNT_ROBOT_02",
    "A_MOUNT_ROBOT_03",
    "A_MOUNT_ROBOT_10",
    "B_MOUNT_ROBOT_01",
    "B_MOUNT_ROBOT_02",
    "B_MOUNT_ROBOT_03",
    "B_MOUNT_ROBOT_10",
];
pub const LIFT_ORIGIN_POINT_CHAINS: [(u16, u16, &str); 11] = [
    (300, 301, "GM_Elevator_01_01"),
    (300, 302, "GM_Elevator_02_01"),
    (301, 300, "GM_Elevator_03_01"),
    (301, 302, "GM_Elevator_04_01"),
    (302, 300, "GM_Elevator_05_01"),
    (302, 301, "GM_Elevator_06_01"),
    (400, 401, "GM_Elevator_07_01"),
    (401, 400, "GM_Elevator_08_01"),
    (500, 501, "GM_Elevator_09_01"),
    (501, 500, "GM_Elevator_10_01"),
    (600, 601, "GM_Elevator_11_01"),
];
pub const HOST_LIFT_BUSY_WAIT_MS: u64 = 5_000;
pub const HOST_LIFT_PLACE_SETTLE_MS: u64 = 300;
pub const HOST_LIFT_RIDE_WATCH_MS: u64 = 60_000;
pub const GUEST_LIFT_ANSWER_WAIT_MS: u64 = 10_000;
pub const GUEST_LIFT_ARRIVAL_WATCH_MS: u64 = 60_000;
pub const GUEST_LIFT_CALLED_TEXT: &str = "Lift called";
pub const GUEST_LIFT_NOT_STARTED_TEXT: &str = "The host could not start the lift";
pub const HOST_PARTNER_CALLED_LIFT_TEXT: &str = "Your partner called the lift";
