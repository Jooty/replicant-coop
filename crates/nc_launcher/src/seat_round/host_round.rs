use std::path::{Path, PathBuf};

use nc_launcher_core::{HostStartGate, LauncherEvent, LauncherNet, NetState, StartGateStep};

use crate::saves::{
    ArrivedSeat, Campaign, SlotSummary, campaign_card_of, campaign_for_host_slot,
    keep_arrived_seat_aside, read_campaign_seat, store_arrived_seat, unix_seconds_now,
};
use crate::seat_round::{HOST_MEMBER_SLOT, shown_campaign_id};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayingCampaign {
    pub host_slot: u8,
    pub campaign_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostStartStep {
    LaunchNow,
    Waiting,
    Stop(String),
}

/// The host's side of the seat round: the card for the picked slot, the start held until the seat
/// is with the guest, and the seats that come back during play.
pub struct HostSeatRound {
    coop_file: PathBuf,
    card_for: Option<(SlotSummary, Campaign)>,
    playing: Option<PlayingCampaign>,
    gate: Option<HostStartGate>,
    pub log: Vec<String>,
}

impl HostSeatRound {
    pub fn new(coop_file: PathBuf) -> Self {
        Self {
            coop_file,
            card_for: None,
            playing: None,
            gate: None,
            log: Vec::new(),
        }
    }

    pub fn coop_file(&self) -> &Path {
        &self.coop_file
    }

    pub fn card_slot(&self) -> Option<u8> {
        self.card_for.as_ref().map(|(slot, _)| slot.index)
    }

    pub fn playing(&self) -> Option<&PlayingCampaign> {
        self.playing.as_ref()
    }

    pub fn waiting_for_seat(&self) -> bool {
        self.gate.is_some()
    }

    /// Makes or reads the slot's campaign and sends its card to the guests.
    pub fn pick(&mut self, net: &dyn LauncherNet, slot: &SlotSummary) {
        match campaign_for_host_slot(&self.coop_file, slot) {
            Ok(Some(campaign)) => {
                net.send_campaign_card(campaign_card_of(&campaign, slot));
                self.log.push(format!(
                    "campaign {}: card sent for co-op slot {} ({})",
                    shown_campaign_id(&campaign.id),
                    slot.index,
                    if campaign.seat.is_some() {
                        "the second seat has a save"
                    } else {
                        "the second seat is empty"
                    }
                ));
                self.card_for = Some((slot.clone(), campaign));
            }
            Ok(None) => self.card_for = None,
            Err(error) => self.log.push(format!(
                "could not read or make the campaign for co-op slot {}: {error}",
                slot.index
            )),
        }
    }

    /// Start pressed: the campaign of `slot` is the one played until the next start, and its seat
    /// goes to the guest first when there is one.
    pub fn begin_start(
        &mut self,
        net: &dyn LauncherNet,
        guest_slot: u8,
        slot: &SlotSummary,
        now_seconds: f64,
    ) -> HostStartStep {
        self.gate = None;
        self.pick(net, slot);
        let Some((_, campaign)) = self
            .card_for
            .clone()
            .filter(|(picked, _)| picked.index == slot.index)
        else {
            self.playing = None;
            return HostStartStep::Stop(
                "The campaign for this save could not be read or made; see the log. The game was not started."
                    .to_owned(),
            );
        };
        self.playing = Some(PlayingCampaign {
            host_slot: slot.index,
            campaign_id: campaign.id.clone(),
        });
        match read_campaign_seat(&self.coop_file, slot.index) {
            Ok(None) => {
                self.log.push(format!(
                    "campaign {}: the second seat is empty, starting without sending a save",
                    shown_campaign_id(&campaign.id)
                ));
                HostStartStep::LaunchNow
            }
            Ok(Some(_)) if !matches!(net.status().state, NetState::Connected { .. }) => {
                HostStartStep::Stop(
                    "The guest is not connected, so their save cannot be sent. The game was not started."
                        .to_owned(),
                )
            }
            Ok(Some(seat)) => match net.send_seat(guest_slot, seat) {
                Some(transfer) => {
                    self.log.push(format!(
                        "campaign {}: sending the second seat's save (transfer {transfer}), the game starts once it arrived",
                        shown_campaign_id(&campaign.id)
                    ));
                    self.gate = Some(HostStartGate::new(guest_slot, transfer, now_seconds));
                    HostStartStep::Waiting
                }
                None => HostStartStep::Stop(
                    "The guest's save could not be sent: the network is not running. The game was not started."
                        .to_owned(),
                ),
            },
            Err(error) => HostStartStep::Stop(format!(
                "The guest's save stored for this campaign failed its checks ({error}). It was kept and not sent. The game was not started."
            )),
        }
    }

    pub fn on_event(
        &mut self,
        net: &dyn LauncherNet,
        event: &LauncherEvent,
    ) -> Option<StartGateStep> {
        let step = self.gate.map(|gate| gate.on_event(event));
        if step
            .as_ref()
            .is_some_and(|step| *step != StartGateStep::Wait)
        {
            self.gate = None;
        }
        match event {
            LauncherEvent::SeatReceived { slot, seat, .. } if *slot != HOST_MEMBER_SLOT => {
                self.on_arrived(net, *slot, seat);
            }
            LauncherEvent::Failed { .. } | LauncherEvent::Stopped => {
                self.playing = None;
                self.card_for = None;
            }
            _ => {}
        }
        step.filter(|step| *step != StartGateStep::Wait)
    }

    pub fn tick(&mut self, now_seconds: f64) -> Option<StartGateStep> {
        let step = self.gate?.on_tick(now_seconds);
        if step == StartGateStep::Wait {
            return None;
        }
        self.gate = None;
        Some(step)
    }

    fn on_arrived(&mut self, net: &dyn LauncherNet, from: u8, seat: &[u8]) {
        let now = unix_seconds_now();
        let Some(playing) = self.playing.clone() else {
            let kept = keep_arrived_seat_aside(&self.coop_file, "unclaimed", seat, now);
            self.log.push(match kept {
                Ok(path) => format!(
                    "a save arrived from member {from} while no campaign is being played; it was kept at {}",
                    path.display()
                ),
                Err(error) => format!(
                    "a save arrived from member {from} while no campaign is being played and could not be kept: {error}"
                ),
            });
            return;
        };
        let id = shown_campaign_id(&playing.campaign_id).to_owned();
        match store_arrived_seat(
            &self.coop_file,
            playing.host_slot,
            &playing.campaign_id,
            seat,
            now,
        ) {
            Ok(ArrivedSeat::Stored {
                unreadable_set_aside,
            }) => {
                self.log.push(format!(
                    "campaign {id}: the second seat's save came back and is stored"
                ));
                if let Some(path) = unreadable_set_aside {
                    self.log.push(format!(
                        "campaign {id}: the seat stored before failed its checks and was kept at {}",
                        path.display()
                    ));
                }
                if let Some((slot, _)) = self.card_for.clone()
                    && slot.index == playing.host_slot
                {
                    self.pick(net, &slot);
                }
            }
            Ok(ArrivedSeat::KeptAside { path, reason }) => self.log.push(format!(
                "campaign {id}: kept both copies, the save that came back was not stored ({reason}); it is at {}",
                path.display()
            )),
            Err(error) => self.log.push(format!(
                "campaign {id}: refused a save that came back: {error}; the stored seat is unchanged"
            )),
        }
    }
}
