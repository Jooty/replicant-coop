use replicant_proto::LiftCallOutcome;

use crate::lift_rides::{GUEST_LIFT_ANSWER_WAIT_MS, GUEST_LIFT_ARRIVAL_WATCH_MS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuestCallStep {
    Send,
    DroppedOneInFlight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuestRideFacts {
    pub now_ms: u64,
    pub phase: Option<u16>,
    pub moved_to_the_host_just_now: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuestRideEvent {
    NoAnswer { waited_ms: u64 },
    Refused(LiftCallOutcome),
    HostRiding,
    ArrivedInNewMap { waited_ms: u64 },
    BroughtAlong { waited_ms: u64 },
    LostTrack { waited_ms: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Asked,
    HostRiding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Call {
    phase: u16,
    point: u16,
    since_ms: u64,
    stage: Stage,
}

/// The guest's side of a lift it called: one call at a time, until the host answers and the ride
/// takes the guest along (a pull inside the map, or the host's map change), or time runs out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GuestLiftCall {
    call: Option<Call>,
}

impl GuestLiftCall {
    pub const fn new() -> Self {
        Self { call: None }
    }

    pub fn call(&mut self, now_ms: u64, phase: u16, point: u16) -> GuestCallStep {
        if self.call.is_some() {
            return GuestCallStep::DroppedOneInFlight;
        }
        self.call = Some(Call {
            phase,
            point,
            since_ms: now_ms,
            stage: Stage::Asked,
        });
        GuestCallStep::Send
    }

    pub fn answered(
        &mut self,
        now_ms: u64,
        point: u16,
        outcome: LiftCallOutcome,
    ) -> Option<GuestRideEvent> {
        let call = self
            .call
            .as_mut()
            .filter(|call| call.point == point && call.stage == Stage::Asked)?;
        if outcome == LiftCallOutcome::Riding {
            call.stage = Stage::HostRiding;
            call.since_ms = now_ms;
            return Some(GuestRideEvent::HostRiding);
        }
        self.call = None;
        Some(GuestRideEvent::Refused(outcome))
    }

    pub fn tick(&mut self, facts: &GuestRideFacts) -> Option<GuestRideEvent> {
        let call = self.call?;
        let waited_ms = facts.now_ms.saturating_sub(call.since_ms);
        let event = match call.stage {
            Stage::Asked if waited_ms >= GUEST_LIFT_ANSWER_WAIT_MS => {
                GuestRideEvent::NoAnswer { waited_ms }
            }
            Stage::Asked => return None,
            Stage::HostRiding if facts.phase.is_some_and(|phase| phase != call.phase) => {
                GuestRideEvent::ArrivedInNewMap { waited_ms }
            }
            Stage::HostRiding if facts.moved_to_the_host_just_now => {
                GuestRideEvent::BroughtAlong { waited_ms }
            }
            Stage::HostRiding if waited_ms >= GUEST_LIFT_ARRIVAL_WATCH_MS => {
                GuestRideEvent::LostTrack { waited_ms }
            }
            Stage::HostRiding => return None,
        };
        self.call = None;
        Some(event)
    }

    pub fn in_flight(&self) -> bool {
        self.call.is_some()
    }
}
