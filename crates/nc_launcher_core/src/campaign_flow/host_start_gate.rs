use crate::LauncherEvent;
use crate::campaign_flow::{SEAT_DELIVERY_TIMEOUT_SECONDS, seat_refusal_words};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartGateStep {
    Wait,
    Launch,
    Stop(String),
}

/// The host's start, held until the guest's launcher has the seat whole and valid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HostStartGate {
    guest_slot: u8,
    transfer: u32,
    began_at_seconds: f64,
}

impl HostStartGate {
    pub fn new(guest_slot: u8, transfer: u32, now_seconds: f64) -> Self {
        Self {
            guest_slot,
            transfer,
            began_at_seconds: now_seconds,
        }
    }

    pub fn transfer(&self) -> u32 {
        self.transfer
    }

    pub fn on_event(&self, event: &LauncherEvent) -> StartGateStep {
        match event {
            LauncherEvent::SeatDelivered { slot, transfer }
                if *slot == self.guest_slot && *transfer == self.transfer =>
            {
                StartGateStep::Launch
            }
            LauncherEvent::SeatTransferRefused {
                slot,
                transfer,
                reason,
                by_peer,
            } if *slot == self.guest_slot && *transfer == self.transfer => {
                let who = if *by_peer {
                    "The guest's launcher refused the save"
                } else {
                    "This launcher could not send the save"
                };
                StartGateStep::Stop(format!(
                    "{who} ({}). The game was not started.",
                    seat_refusal_words(*reason)
                ))
            }
            LauncherEvent::MemberLeft { slot, .. } if *slot == self.guest_slot => {
                StartGateStep::Stop(
                    "The guest left before their save arrived. The game was not started."
                        .to_owned(),
                )
            }
            LauncherEvent::Failed { .. } | LauncherEvent::Stopped => StartGateStep::Stop(
                "The connection was lost before the guest's save arrived. The game was not started."
                    .to_owned(),
            ),
            _ => StartGateStep::Wait,
        }
    }

    pub fn on_tick(&self, now_seconds: f64) -> StartGateStep {
        if now_seconds - self.began_at_seconds >= SEAT_DELIVERY_TIMEOUT_SECONDS {
            return StartGateStep::Stop(format!(
                "The guest's save did not arrive within {SEAT_DELIVERY_TIMEOUT_SECONDS:.0} s. The game was not started."
            ));
        }
        StartGateStep::Wait
    }
}
