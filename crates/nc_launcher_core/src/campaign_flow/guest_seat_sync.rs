use replicant_game::story_progress::{SeatError, changed_slot, seat_bytes_of};

const SLOT_WHEN_THE_CHANGE_IS_UNCLEAR: u8 = 0;

/// The guest's side of one campaign while it plays: which seat must go back to the host, which is
/// on its way, and which the host has confirmed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuestSeatSync {
    campaign_id: String,
    baseline_file: Vec<u8>,
    delivered: Option<Vec<u8>>,
    pending: Option<Vec<u8>>,
    in_flight: Option<(u32, Vec<u8>)>,
}

impl GuestSeatSync {
    /// `session_file` is the file as the game was booted into it; `delivered` the seat the host is
    /// known to hold; `pending` a seat that has still to reach the host.
    pub fn new(
        campaign_id: String,
        session_file: Vec<u8>,
        delivered: Option<Vec<u8>>,
        pending: Option<Vec<u8>>,
    ) -> Self {
        let pending = pending.filter(|seat| delivered.as_ref() != Some(seat));
        Self {
            campaign_id,
            baseline_file: session_file,
            delivered,
            pending,
            in_flight: None,
        }
    }

    pub fn campaign_id(&self) -> &str {
        &self.campaign_id
    }

    pub fn pending(&self) -> Option<&[u8]> {
        self.pending.as_deref()
    }

    /// The game finished a save: the seat to keep as unsent and send, if it changed.
    pub fn on_saved(&mut self, file_now: &[u8]) -> Result<Option<Vec<u8>>, SeatError> {
        let slot =
            changed_slot(&self.baseline_file, file_now).unwrap_or(SLOT_WHEN_THE_CHANGE_IS_UNCLEAR);
        let seat = seat_bytes_of(file_now, slot)?;
        self.baseline_file = file_now.to_vec();
        if self.delivered.as_ref() != Some(&seat) {
            self.pending = Some(seat);
        }
        Ok(self.to_send())
    }

    /// The pending seat when it is not already on its way.
    pub fn to_send(&self) -> Option<Vec<u8>> {
        let pending = self.pending.as_ref()?;
        let already_going = self
            .in_flight
            .as_ref()
            .is_some_and(|(_, seat)| seat == pending);
        (!already_going).then(|| pending.clone())
    }

    pub fn sending(&mut self, transfer: u32, seat: Vec<u8>) {
        self.in_flight = Some((transfer, seat));
    }

    /// The host confirmed a transfer: the seat it holds now, for the hidden copy.
    pub fn on_delivered(&mut self, transfer: u32) -> Option<Vec<u8>> {
        let (sent, seat) = self.in_flight.take()?;
        if sent != transfer {
            self.in_flight = Some((sent, seat));
            return None;
        }
        if self.pending.as_ref() == Some(&seat) {
            self.pending = None;
        }
        self.delivered = Some(seat.clone());
        Some(seat)
    }

    pub fn on_refused(&mut self, transfer: u32) {
        if self
            .in_flight
            .as_ref()
            .is_some_and(|(sent, _)| *sent == transfer)
        {
            self.in_flight = None;
        }
    }

    pub fn link_lost(&mut self) {
        self.in_flight = None;
    }
}
