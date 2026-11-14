use replicant_proto::{LiftCall, LiftCallOutcome};

use crate::lift_rides::{
    HOST_LIFT_BUSY_WAIT_MS, HOST_LIFT_PLACE_SETTLE_MS, HOST_LIFT_RIDE_WATCH_MS, lift_ride,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostBusy {
    Scene,
    Talk,
    ScriptPause,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostRideFacts {
    pub now_ms: u64,
    pub phase: Option<u16>,
    pub busy: Option<HostBusy>,
    pub menu_open: bool,
    pub script_pause: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RideEnd {
    PauseEnded,
    NewMap,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HostRideStep {
    Idle,
    Wait(HostBusy),
    CloseMenu,
    WaitForMenu,
    Place([f32; 3]),
    Settling,
    Run {
        phase: u16,
        point: u16,
        origin: u16,
    },
    Refuse {
        phase: u16,
        point: u16,
        outcome: LiftCallOutcome,
    },
    Riding,
    Arrived {
        end: RideEnd,
        waited_ms: u64,
    },
    LostTrack {
        waited_ms: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Stage {
    Waiting { menu_close_asked: bool },
    Placed { at_ms: u64 },
    Riding { since_ms: u64, saw_pause: bool },
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Ride {
    call: LiftCall,
    received_ms: u64,
    stage: Stage,
}

/// The host's side of a lift the guest called: waits until the host's game can take it, moves the
/// host to the guest at the panel, runs the guest's pick, then watches the ride to its end.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HostLiftRide {
    ride: Option<Ride>,
}

impl HostLiftRide {
    pub const fn new() -> Self {
        Self { ride: None }
    }

    pub fn receive(
        &mut self,
        call: LiftCall,
        host_phase: Option<(u16, &str)>,
        now_ms: u64,
    ) -> Result<(), LiftCallOutcome> {
        if self.ride.is_some() {
            return Err(LiftCallOutcome::AnotherRideRunning);
        }
        let Some((phase, phase_name)) = host_phase.filter(|(phase, _)| *phase == call.phase) else {
            return Err(LiftCallOutcome::OtherMap);
        };
        if lift_ride(phase_name, i32::from(call.point), i32::from(call.origin)).is_none() {
            return Err(LiftCallOutcome::NotALift);
        }
        self.ride = Some(Ride {
            call: LiftCall { phase, ..call },
            received_ms: now_ms,
            stage: Stage::Waiting {
                menu_close_asked: false,
            },
        });
        Ok(())
    }

    pub fn step(&mut self, facts: &HostRideFacts) -> HostRideStep {
        let Some(ride) = self.ride.as_mut() else {
            return HostRideStep::Idle;
        };
        let call = ride.call;
        match ride.stage {
            Stage::Waiting { .. } | Stage::Placed { .. } if facts.phase != Some(call.phase) => {
                self.ride = None;
                HostRideStep::Refuse {
                    phase: call.phase,
                    point: call.point,
                    outcome: LiftCallOutcome::OtherMap,
                }
            }
            Stage::Waiting { .. } | Stage::Placed { .. }
                if facts.now_ms.saturating_sub(ride.received_ms) >= HOST_LIFT_BUSY_WAIT_MS =>
            {
                self.ride = None;
                HostRideStep::Refuse {
                    phase: call.phase,
                    point: call.point,
                    outcome: LiftCallOutcome::HostBusy,
                }
            }
            Stage::Waiting { .. } | Stage::Placed { .. } if let Some(busy) = facts.busy => {
                HostRideStep::Wait(busy)
            }
            Stage::Waiting { menu_close_asked } if facts.menu_open => {
                if menu_close_asked {
                    return HostRideStep::WaitForMenu;
                }
                ride.stage = Stage::Waiting {
                    menu_close_asked: true,
                };
                HostRideStep::CloseMenu
            }
            Stage::Waiting { .. } => {
                ride.stage = Stage::Placed {
                    at_ms: facts.now_ms,
                };
                HostRideStep::Place(call.position)
            }
            Stage::Placed { at_ms }
                if facts.now_ms.saturating_sub(at_ms) < HOST_LIFT_PLACE_SETTLE_MS =>
            {
                HostRideStep::Settling
            }
            Stage::Placed { .. } => {
                ride.stage = Stage::Riding {
                    since_ms: facts.now_ms,
                    saw_pause: false,
                };
                HostRideStep::Run {
                    phase: call.phase,
                    point: call.point,
                    origin: call.origin,
                }
            }
            Stage::Riding {
                since_ms,
                saw_pause,
            } => {
                let waited_ms = facts.now_ms.saturating_sub(since_ms);
                let end = if facts.phase.is_some_and(|phase| phase != call.phase) {
                    Some(RideEnd::NewMap)
                } else if saw_pause && !facts.script_pause {
                    Some(RideEnd::PauseEnded)
                } else {
                    None
                };
                if let Some(end) = end {
                    self.ride = None;
                    return HostRideStep::Arrived { end, waited_ms };
                }
                if waited_ms >= HOST_LIFT_RIDE_WATCH_MS {
                    self.ride = None;
                    return HostRideStep::LostTrack { waited_ms };
                }
                ride.stage = Stage::Riding {
                    since_ms,
                    saw_pause: saw_pause || facts.script_pause,
                };
                HostRideStep::Riding
            }
        }
    }

    pub fn in_flight(&self) -> bool {
        self.ride.is_some()
    }
}
